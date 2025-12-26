use chrono::Utc;
use clap::Parser;
use dotenvy::dotenv;
use rand::Rng;
use reqwest::{StatusCode, blocking::Client};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sr_common::db::{
    create_pool_from_url_checked, fetch_email_body, lock_next_pending_job, run_migrations,
    upsert_extraction_job,
};
use sr_common::queue::{
    ExtractionJob, ExtractionQueue, FinalMethod, JobError, JobOutcome, QueueStatus,
    RecommendedMethod,
};
use std::thread::sleep as std_sleep;
use std::time::Duration as StdDuration;
use tokio::time::{Duration, sleep};
use tracing::{error, info};

#[derive(Debug, Clone, PartialEq, Eq)]
enum CompareMode {
    None,
    Shadow,
}

fn provider_defaults(provider: &str) -> (String, String) {
    match provider.to_ascii_lowercase().as_str() {
        "openai" => (
            "gpt-4o-mini".into(),
            "https://api.openai.com/v1/chat/completions".into(),
        ),
        "anthropic" => (
            "claude-3-5-sonnet-20240620".into(),
            "https://api.anthropic.com/v1/messages".into(),
        ),
        "google" | "google-genai" => (
            "gemini-1.5-flash".into(),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent"
                .into(),
        ),
        "mistral" => (
            "mistral-large-latest".into(),
            "https://api.mistral.ai/v1/chat/completions".into(),
        ),
        "xai" => (
            "grok-2-latest".into(),
            "https://api.x.ai/v1/chat/completions".into(),
        ),
        "huggingface" | "hf" => (
            "meta-llama/Meta-Llama-3-70B-Instruct".into(),
            "https://api-inference.huggingface.co/models/meta-llama/Meta-Llama-3-70B-Instruct".into(),
        ),
        _ => (
            "deepseek-chat".into(),
            "http://localhost:8000/api/v1/extract".into(),
        ),
    }
}

#[derive(Debug, Clone)]
struct LlmRuntimeConfig {
    enabled: bool,
    provider: String,
    model: String,
    endpoint: String,
    api_key: String,
    timeout_secs: u64,
    max_retries: u32,
    retry_backoff_secs: u64,
    compare_mode: CompareMode,
    primary_provider: String,
    shadow_provider: String,
    shadow_sample_percent: u8,
    shadow_api_key: String,
    shadow_endpoint: Option<String>,
    shadow_model: Option<String>,
}

impl Default for LlmRuntimeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            provider: "deepseek".into(),
            model: "deepseek-chat".into(),
            endpoint: "http://localhost:8000/api/v1/extract".into(),
            api_key: String::new(),
            timeout_secs: 30,
            max_retries: 3,
            retry_backoff_secs: 5,
            compare_mode: CompareMode::None,
            primary_provider: "deepseek".into(),
            shadow_provider: "openai".into(),
            shadow_sample_percent: 10,
            shadow_api_key: String::new(),
            shadow_endpoint: None,
            shadow_model: None,
        }
    }
}

impl LlmRuntimeConfig {
    fn from_env() -> Self {
        fn provider_api_key(provider: &str) -> Option<String> {
            let lower = provider.to_ascii_lowercase();
            match lower.as_str() {
                "openai" => std::env::var("OPENAI_API_KEY").ok(),
                "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
                "google" | "google-genai" => std::env::var("GOOGLE_API_KEY").ok(),
                "mistral" => std::env::var("MISTRAL_API_KEY").ok(),
                "xai" => std::env::var("XAI_API_KEY").ok(),
                "huggingface" | "hf" => std::env::var("HUGGINGFACE_API_KEY")
                    .ok()
                    .or_else(|| std::env::var("HF_TOKEN").ok()),
                _ => None,
            }
        }

        fn parse_bool(key: &str, default: bool) -> bool {
            match std::env::var(key) {
                Ok(val) => matches!(
                    val.to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                ),
                Err(_) => default,
            }
        }

        fn parse_u64(key: &str, default: u64) -> u64 {
            std::env::var(key)
                .ok()
                .and_then(|raw| raw.parse::<u64>().ok())
                .unwrap_or(default)
        }

        fn parse_u32(key: &str, default: u32) -> u32 {
            std::env::var(key)
                .ok()
                .and_then(|raw| raw.parse::<u32>().ok())
                .unwrap_or(default)
        }

        fn parse_sample_percent() -> u8 {
            std::env::var("LLM_SHADOW_SAMPLE_PERCENT")
                .ok()
                .and_then(|raw| raw.parse::<u8>().ok())
                .unwrap_or(10)
                .min(100)
        }

        let compare_mode = std::env::var("LLM_COMPARE_MODE")
            .unwrap_or_else(|_| "none".into())
            .to_ascii_lowercase();

        let provider = std::env::var("LLM_PROVIDER").unwrap_or_else(|_| "deepseek".into());
        let (default_model, default_endpoint) = provider_defaults(&provider);
        let primary_provider =
            std::env::var("LLM_PRIMARY_PROVIDER").unwrap_or_else(|_| provider.clone());
        let shadow_provider =
            std::env::var("LLM_SHADOW_PROVIDER").unwrap_or_else(|_| "openai".into());

        let api_key = std::env::var("LLM_API_KEY")
            .ok()
            .or_else(|| provider_api_key(&provider))
            .unwrap_or_default();

        let shadow_api_key = std::env::var("LLM_SHADOW_API_KEY")
            .ok()
            .or_else(|| provider_api_key(&shadow_provider))
            .or_else(|| {
                if shadow_provider.eq_ignore_ascii_case(&provider) && !api_key.is_empty() {
                    Some(api_key.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let enabled = parse_bool("LLM_ENABLED", true);
        if enabled && api_key.is_empty() {
            panic!("LLM_API_KEY is required when LLM_ENABLED=true");
        }

        Self {
            enabled,
            provider: provider.clone(),
            model: std::env::var("LLM_MODEL").unwrap_or_else(|_| default_model),
            endpoint: std::env::var("LLM_ENDPOINT").unwrap_or_else(|_| default_endpoint),
            api_key,
            timeout_secs: parse_u64("LLM_TIMEOUT_SECONDS", 30),
            max_retries: parse_u32("LLM_MAX_RETRIES", 3),
            retry_backoff_secs: parse_u64("LLM_RETRY_BACKOFF_SECONDS", 5),
            compare_mode: match compare_mode.as_str() {
                "shadow" => CompareMode::Shadow,
                _ => CompareMode::None,
            },
            primary_provider,
            shadow_provider,
            shadow_sample_percent: parse_sample_percent(),
            shadow_api_key,
            shadow_endpoint: std::env::var("LLM_SHADOW_ENDPOINT").ok(),
            shadow_model: std::env::var("LLM_SHADOW_MODEL").ok(),
        }
    }
}

#[derive(Debug, Clone)]
struct ShadowCompareConfig {
    mode: CompareMode,
    sample_percent: u8,
    shadow_provider: String,
    primary_provider: String,
}

#[derive(Debug, Serialize)]
struct LlmRequest {
    message_id: String,
    source_text: String,
    extractor_hints: serde_json::Value,
    model: String,
    timeout_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct LlmResponse {
    #[serde(default)]
    message_id: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    extracted: serde_json::Value,
    #[serde(default)]
    missing_fields: Vec<String>,
    #[serde(default)]
    requires_manual_review: bool,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    latency_ms: Option<i32>,
    #[serde(default)]
    model_used: Option<String>,
}

#[derive(Debug, Parser)]
#[command(
    name = "sr-llm-worker",
    about = "Process LLM recommended extraction jobs"
)]
struct Cli {
    /// PostgreSQL connection string
    #[arg(long, env = "DATABASE_URL")]
    db_url: String,

    /// Worker id recorded into the queue
    #[arg(long, default_value = "sr-llm-worker")]
    worker_id: String,

    /// Optional cap on how many jobs to process in one run (default: until queue is empty)
    #[arg(long)]
    max_jobs: Option<usize>,

    /// Continue polling instead of exiting immediately when the queue is empty (default: keep polling)
    #[arg(long, default_value_t = false)]
    exit_on_empty: bool,

    /// Idle poll interval in milliseconds when running as a long-lived service
    #[arg(long, default_value_t = 5000)]
    idle_poll_interval_ms: u64,
}

pub fn run_sample_flow_with_worker(worker_id: &str) -> ExtractionQueue {
    let mut queue = ExtractionQueue::default();
    let llm_config = LlmRuntimeConfig::from_env();
    let shadow_config = shadow_config_from_env(&llm_config);

    let mut job = ExtractionJob::new(
        "llm-message-1",
        "llm subject",
        Utc::now(),
        "llm-subject-hash",
    );
    job.recommended_method = Some(RecommendedMethod::LlmRecommended);

    queue.enqueue(job);

    let sample_body = "sample body";
    queue.process_next_with_worker(worker_id, |job| {
        handle_llm_job(job, sample_body, &llm_config)
    });

    if let Some(processed) = queue.jobs.first() {
        if processed.canary_target {
            let _ = spawn_shadow_compare(
                processed.clone(),
                sample_body.to_string(),
                &llm_config,
                &shadow_config,
            );
        }
    }

    queue
}

pub fn run_sample_flow() -> ExtractionQueue {
    run_sample_flow_with_worker("sr-llm-worker")
}

fn shadow_config_from_env(config: &LlmRuntimeConfig) -> ShadowCompareConfig {
    ShadowCompareConfig {
        mode: config.compare_mode.clone(),
        sample_percent: config.shadow_sample_percent,
        shadow_provider: config.shadow_provider.clone(),
        primary_provider: config.primary_provider.clone(),
    }
}

fn should_sample_shadow(sample_percent: u8) -> bool {
    if sample_percent == 0 {
        return false;
    }

    rand::thread_rng().gen_ratio(u32::from(sample_percent), 100)
}

fn build_extractor_hints(partial_fields: &Option<serde_json::Value>) -> serde_json::Value {
    let mut hints = serde_json::Map::new();
    let Some(obj) = partial_fields.as_ref().and_then(|v| v.as_object()) else {
        return serde_json::Value::Object(hints);
    };

    if let Some(name) = obj.get("project_name").and_then(|v| v.as_str()) {
        hints.insert("project_name".to_string(), json!(name));
    }
    if let Some(min) = obj.get("monthly_tanka_min").and_then(|v| v.as_i64()) {
        hints.insert("monthly_tanka_min".to_string(), json!(min));
    }
    if let Some(max) = obj.get("monthly_tanka_max").and_then(|v| v.as_i64()) {
        hints.insert("monthly_tanka_max".to_string(), json!(max));
    }
    if let Some(skills) = obj
        .get("required_skills_keywords")
        .and_then(|v| v.as_array())
    {
        hints.insert("required_skills_keywords".to_string(), json!(skills));
    }
    if let Some(remote) = obj.get("remote_onsite").and_then(|v| v.as_str()) {
        hints.insert("remote_onsite".to_string(), json!(remote));
    }
    if let Some(todofuken) = obj.get("work_todofuken").and_then(|v| v.as_str()) {
        hints.insert("work_todofuken".to_string(), json!(todofuken));
    }
    if let Some(years) = obj.get("min_experience_years").and_then(|v| v.as_f64()) {
        hints.insert("min_experience_years".to_string(), json!(years));
    }
    if let Some(reason) = obj.get("decision_reason").and_then(|v| v.as_str()) {
        hints.insert("_rust_decision_reason".to_string(), json!(reason));
    }
    if let Some(hash) = obj.get("subject_hash").and_then(|v| v.as_str()) {
        hints.insert("_subject_hash".to_string(), json!(hash));
    }

    serde_json::Value::Object(hints)
}

fn build_llm_request(
    job: &ExtractionJob,
    body_text: &str,
    config: &LlmRuntimeConfig,
) -> LlmRequest {
    LlmRequest {
        message_id: job.message_id.clone(),
        source_text: body_text.to_string(),
        extractor_hints: build_extractor_hints(&job.partial_fields),
        model: config.model.clone(),
        timeout_seconds: config.timeout_secs,
    }
}

fn is_retryable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::TOO_MANY_REQUESTS
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
            | StatusCode::INTERNAL_SERVER_ERROR
    )
}

fn perform_llm_request(
    config: &LlmRuntimeConfig,
    endpoint: &str,
    api_key: &str,
    request: &LlmRequest,
) -> Result<LlmResponse, JobError> {
    // Disable environment proxies so local mock servers (used in tests) are hit directly
    // instead of being tunneled through corporate MITM proxies that would block the request.
    let client = Client::builder()
        .no_proxy()
        .timeout(StdDuration::from_secs(config.timeout_secs))
        .build()
        .map_err(|err| JobError::Retryable {
            message: format!("failed to build http client: {err}"),
            retry_after: None,
        })?;

    for attempt in 0..=config.max_retries {
        let response = client
            .post(endpoint)
            .bearer_auth(api_key)
            .json(request)
            .send();

        match response {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    return resp
                        .json::<LlmResponse>()
                        .map_err(|err| JobError::Permanent {
                            message: format!("invalid llm response body: {err}"),
                        });
                }

                if is_retryable_status(status) && attempt < config.max_retries {
                    std_sleep(StdDuration::from_secs(config.retry_backoff_secs));
                    continue;
                }

                let body = resp.text().unwrap_or_default();
                let message = format!("llm call failed with status {status}: {body}");
                if is_retryable_status(status) {
                    return Err(JobError::Retryable {
                        message,
                        retry_after: Some(chrono::Duration::seconds(
                            config.retry_backoff_secs as i64,
                        )),
                    });
                }

                return Err(JobError::Permanent { message });
            }
            Err(err) => {
                if attempt < config.max_retries {
                    std_sleep(StdDuration::from_secs(config.retry_backoff_secs));
                    continue;
                }

                return Err(JobError::Retryable {
                    message: format!("llm request error: {err}"),
                    retry_after: Some(chrono::Duration::seconds(config.retry_backoff_secs as i64)),
                });
            }
        }
    }

    Err(JobError::Retryable {
        message: "llm retries exhausted".into(),
        retry_after: Some(chrono::Duration::seconds(config.retry_backoff_secs as i64)),
    })
}

fn mark_shadow_canary(job: &mut ExtractionJob, config: &ShadowCompareConfig) -> bool {
    if config.mode == CompareMode::Shadow && should_sample_shadow(config.sample_percent) {
        job.canary_target = true;
        return true;
    }

    job.canary_target = false;
    false
}

fn shadow_endpoint_and_model(config: &LlmRuntimeConfig) -> (String, String) {
    if let (Some(endpoint), Some(model)) = (&config.shadow_endpoint, &config.shadow_model) {
        return (endpoint.clone(), model.clone());
    }

    let (default_model, default_endpoint) = provider_defaults(&config.shadow_provider);
    (
        config.shadow_endpoint.clone().unwrap_or(default_endpoint),
        config.shadow_model.clone().unwrap_or(default_model),
    )
}

fn spawn_shadow_compare(
    job: ExtractionJob,
    body_text: String,
    config: &LlmRuntimeConfig,
    shadow: &ShadowCompareConfig,
) -> Option<tokio::task::JoinHandle<()>> {
    if shadow.mode != CompareMode::Shadow || !job.canary_target {
        return None;
    }

    let (shadow_endpoint, shadow_model) = shadow_endpoint_and_model(config);
    let shadow_api_key = if config.shadow_api_key.is_empty() {
        config.api_key.clone()
    } else {
        config.shadow_api_key.clone()
    };

    if shadow_api_key.is_empty() {
        info!(
            message_id = %job.message_id,
            %shadow_endpoint,
            %shadow_model,
            "skipping shadow comparison because no API key is configured",
        );
        return None;
    }
    let shadow_provider = shadow.shadow_provider.clone();
    let primary_provider = shadow.primary_provider.clone();
    let mut shadow_config = config.clone();
    shadow_config.model = shadow_model;

    Some(tokio::task::spawn_blocking(move || {
        let request = build_llm_request(&job, &body_text, &shadow_config);
        match perform_llm_request(&shadow_config, &shadow_endpoint, &shadow_api_key, &request) {
            Ok(shadow_resp) => {
                let diff =
                    if shadow_resp.extracted == job.partial_fields.clone().unwrap_or_default() {
                        "match"
                    } else {
                        "diff"
                    };
                info!(
                    message_id = %job.message_id,
                    %shadow_provider,
                    %primary_provider,
                    diff,
                    shadow_model_used = shadow_resp.model_used,
                    shadow_latency_ms = shadow_resp.latency_ms,
                    "shadow comparison completed",
                );
            }
            Err(err) => {
                let err_message = match err {
                    JobError::Retryable { ref message, .. } => message.clone(),
                    JobError::Permanent { ref message } => message.clone(),
                };
                info!(
                    message_id = %job.message_id,
                    %shadow_provider,
                    %primary_provider,
                    error = %err_message,
                    "shadow comparison failed",
                );
            }
        }
    }))
}

fn handle_llm_job(
    job: &ExtractionJob,
    body_text: &str,
    config: &LlmRuntimeConfig,
) -> Result<JobOutcome, JobError> {
    if job.recommended_method != Some(RecommendedMethod::LlmRecommended) {
        return Err(JobError::Permanent {
            message: "non-llm job routed to sr-llm-worker".into(),
        });
    }

    if !config.enabled {
        return Err(JobError::Permanent {
            message: "LLM_DISABLED: LLM_ENABLED=0".into(),
        });
    }

    if config.api_key.is_empty() {
        return Err(JobError::Permanent {
            message: format!(
                "missing LLM_API_KEY (or vendor key) for provider {}",
                config.provider
            ),
        });
    }

    let request = build_llm_request(job, body_text, config);
    let started = Utc::now();
    let response = perform_llm_request(config, &config.endpoint, &config.api_key, &request)?;
    let latency = response
        .latency_ms
        .or_else(|| (Utc::now() - started).num_milliseconds().try_into().ok());

    let mut requires_manual_review = response.requires_manual_review;
    let mut decorations: Vec<String> = Vec::new();

    if !response.status.is_empty() {
        decorations.push(format!("status={}", response.status));
        if !matches!(
            response.status.to_ascii_lowercase().as_str(),
            "ok" | "success"
        ) {
            requires_manual_review = true;
        }
    }

    if !response.message_id.is_empty() && response.message_id != job.message_id {
        decorations.push(format!(
            "message_id mismatch (expected {}, got {})",
            job.message_id, response.message_id
        ));
        requires_manual_review = true;
    }

    if !response.missing_fields.is_empty() {
        decorations.push(format!(
            "missing fields: {}",
            response.missing_fields.join(", ")
        ));
        requires_manual_review = true;
    }

    let mut decision_reason = response
        .reason
        .clone()
        .or_else(|| Some(format!("processed by {}", config.provider)));

    if !decorations.is_empty() {
        let suffix = decorations.join("; ");
        decision_reason = Some(match decision_reason {
            Some(existing) if !existing.is_empty() => format!("{existing}; {suffix}"),
            _ => suffix,
        });
    }

    let manual_review_reason = if requires_manual_review {
        decision_reason.clone()
    } else {
        None
    };

    Ok(JobOutcome {
        final_method: FinalMethod::LlmCompleted,
        partial_fields: Some(response.extracted.clone()),
        decision_reason,
        llm_latency_ms: latency,
        requires_manual_review,
        manual_review_reason,
    })
}

fn apply_outcome(
    mut job: ExtractionJob,
    outcome: Result<JobOutcome, JobError>,
) -> (ExtractionJob, QueueStatus) {
    match outcome {
        Ok(outcome) => {
            let finished_at = Utc::now();
            job.status = QueueStatus::Completed;
            job.final_method = Some(outcome.final_method);
            job.partial_fields = outcome.partial_fields;
            job.decision_reason = outcome.decision_reason;
            job.manual_review_reason = outcome.manual_review_reason;
            job.llm_latency_ms = outcome.llm_latency_ms;
            job.completed_at = Some(finished_at);
            job.updated_at = finished_at;
            job.requires_manual_review = outcome.requires_manual_review;
            job.locked_by = None;
        }
        Err(JobError::Permanent { message }) => {
            let finished_at = Utc::now();
            job.status = QueueStatus::Completed;
            job.final_method = Some(FinalMethod::ManualReview);
            job.last_error = Some(message.clone());
            job.decision_reason = Some(message.clone());
            job.manual_review_reason = Some(message);
            job.completed_at = Some(finished_at);
            job.updated_at = finished_at;
            job.requires_manual_review = true;
            job.locked_by = None;
        }
        Err(JobError::Retryable {
            message,
            retry_after,
        }) => {
            let finished_at = Utc::now();
            job.status = QueueStatus::Pending;
            job.retry_count += 1;
            job.next_retry_at =
                Some(finished_at + retry_after.unwrap_or_else(|| chrono::Duration::minutes(5)));
            job.last_error = Some(message);
            job.final_method = None;
            job.partial_fields = None;
            job.decision_reason = None;
            job.manual_review_reason = None;
            job.llm_latency_ms = None;
            job.completed_at = None;
            job.updated_at = finished_at;
            job.locked_by = None;
        }
    }

    let status = job.status.clone();
    (job, status)
}

async fn process_locked_job(
    pool: &sr_common::db::PgPool,
    worker_id: &str,
    mut locked: ExtractionJob,
    llm_config: &LlmRuntimeConfig,
    shadow_config: &ShadowCompareConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let shadow_selected = mark_shadow_canary(&mut locked, shadow_config);
    let body_text = match fetch_email_body(pool, &locked.message_id).await {
        Ok(Some(body)) => body,
        Ok(None) => {
            let (processed, _) = apply_outcome(
                locked.clone(),
                Err(JobError::Permanent {
                    message: "missing source_text in anken_emails".into(),
                }),
            );
            upsert_extraction_job(pool, &processed).await?;
            return Ok(());
        }
        Err(err) => {
            let (processed, _) = apply_outcome(
                locked.clone(),
                Err(JobError::Retryable {
                    message: format!("failed to fetch email body: {err}"),
                    retry_after: Some(chrono::Duration::minutes(5)),
                }),
            );
            upsert_extraction_job(pool, &processed).await?;
            return Ok(());
        }
    };

    let (processed, _status) = apply_outcome(
        locked.clone(),
        handle_llm_job(&locked, &body_text, llm_config),
    );
    let rows = upsert_extraction_job(pool, &processed).await?;
    info!(
        rows,
        worker_id = %worker_id,
        message_id = %processed.message_id,
        "persisted processed job",
    );

    if shadow_selected {
        let _ = spawn_shadow_compare(processed.clone(), body_text, llm_config, shadow_config);
    }

    Ok(())
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let args = Cli::parse();
    let llm_config = LlmRuntimeConfig::from_env();
    let shadow_config = shadow_config_from_env(&llm_config);
    let pool = create_pool_from_url_checked(&args.db_url).await?;
    run_migrations(&pool).await?;
    let status = pool.status();
    info!(
        size = status.size,
        available = status.available,
        worker_id = %args.worker_id,
        llm_enabled = llm_config.enabled,
        llm_provider = %llm_config.provider,
        llm_model = %llm_config.model,
        llm_endpoint = %llm_config.endpoint,
        shadow_mode = ?shadow_config.mode,
        shadow_sample_percent = shadow_config.sample_percent,
        "created postgres connection pool for llm worker",
    );

    let mut processed_jobs = 0usize;
    let max_jobs = args.max_jobs.unwrap_or(usize::MAX);

    while processed_jobs < max_jobs {
        let maybe_job = lock_next_pending_job(&pool, &args.worker_id, Utc::now()).await?;

        let Some(job) = maybe_job else {
            if args.exit_on_empty {
                if processed_jobs == 0 {
                    info!("no pending jobs found; exiting");
                }
                break;
            }

            sleep(Duration::from_millis(args.idle_poll_interval_ms)).await;
            continue;
        };

        process_locked_job(&pool, &args.worker_id, job, &llm_config, &shadow_config).await?;
        processed_jobs += 1;
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        error!(error = %err, "sr-llm-worker failed");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    fn with_env(vars: &[(&str, Option<&str>)], f: impl FnOnce()) {
        use std::sync::Mutex;
        static ENV_GUARD: Mutex<()> = Mutex::new(());
        let _guard = ENV_GUARD.lock().unwrap();

        let prev: Vec<(String, Option<String>)> = vars
            .iter()
            .map(|(key, value)| {
                let previous = std::env::var(key).ok();
                match value {
                    Some(v) => unsafe { std::env::set_var(key, v) },
                    None => unsafe { std::env::remove_var(key) },
                }
                (key.to_string(), previous)
            })
            .collect();

        f();

        for (key, previous) in prev {
            if let Some(v) = previous {
                unsafe { std::env::set_var(&key, v) };
            } else {
                unsafe { std::env::remove_var(&key) };
            }
        }
    }

    #[test]
    fn llm_job_is_marked_completed() {
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(POST).path("/api/v1/extract");
            then.status(200).json_body(json!({
                "extracted": {"project_name": "from-llm"},
                "latency_ms": 222,
                "reason": "llm ok",
            }));
        });

        with_env(
            &[
                ("LLM_ENDPOINT", Some(&server.url("/api/v1/extract"))),
                ("LLM_API_KEY", Some("token")),
            ],
            || {
                let queue = run_sample_flow();

                assert_eq!(queue.jobs.len(), 1);
                let job = &queue.jobs[0];
                assert_eq!(job.final_method, Some(FinalMethod::LlmCompleted));
                assert_eq!(job.status.as_str(), "completed");
                assert!(!job.requires_manual_review);
                assert_eq!(
                    job.partial_fields.as_ref().unwrap()["project_name"],
                    json!("from-llm")
                );
                assert_eq!(job.llm_latency_ms, Some(222));
                assert!(
                    job.decision_reason
                        .as_ref()
                        .map(|r| r.contains("llm ok"))
                        .unwrap_or(false)
                );
            },
        );
    }

    #[test]
    fn llm_missing_fields_trigger_manual_review() {
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(POST).path("/api/v1/extract");
            then.status(200).json_body(json!({
                "message_id": "llm-message-1",
                "extracted": {"project_name": "from-llm"},
                "latency_ms": 120,
                "reason": "partial extraction",
                "missing_fields": ["skills", "years_of_experience"],
                "status": "partial",
            }));
        });

        with_env(
            &[
                ("LLM_ENDPOINT", Some(&server.url("/api/v1/extract"))),
                ("LLM_API_KEY", Some("token")),
            ],
            || {
                let queue = run_sample_flow();

                assert_eq!(queue.jobs.len(), 1);
                let job = &queue.jobs[0];
                assert_eq!(job.final_method, Some(FinalMethod::LlmCompleted));
                assert!(job.requires_manual_review);
                let reason = job.manual_review_reason.as_ref().unwrap();
                assert!(reason.contains("missing fields"));
                assert!(reason.contains("skills"));
                assert!(reason.contains("years_of_experience"));
                assert!(reason.contains("status=partial"));
            },
        );
    }

    #[test]
    fn llm_message_id_mismatch_forces_manual_review() {
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(POST).path("/api/v1/extract");
            then.status(200).json_body(json!({
                "message_id": "unexpected",
                "extracted": {"project_name": "from-llm"},
                "latency_ms": 80,
                "status": "ok",
            }));
        });

        with_env(
            &[
                ("LLM_ENDPOINT", Some(&server.url("/api/v1/extract"))),
                ("LLM_API_KEY", Some("token")),
            ],
            || {
                let queue = run_sample_flow();

                assert_eq!(queue.jobs.len(), 1);
                let job = &queue.jobs[0];
                assert_eq!(job.final_method, Some(FinalMethod::LlmCompleted));
                assert!(job.requires_manual_review);
                let reason = job.manual_review_reason.as_ref().unwrap();
                assert!(reason.contains("message_id mismatch"));
                assert!(reason.contains("unexpected"));
            },
        );
    }

    #[test]
    fn non_llm_jobs_are_sent_to_manual_review() {
        let mut queue = ExtractionQueue::default();
        let mut job = ExtractionJob::new("m2", "subject", Utc::now(), "hash");
        job.recommended_method = Some(RecommendedMethod::RustRecommended);
        let llm_config = LlmRuntimeConfig::default();

        queue.enqueue(job);

        queue.process_next_with_worker("sr-llm-worker", |j| handle_llm_job(j, "body", &llm_config));

        let job = &queue.jobs[0];
        assert_eq!(job.status, QueueStatus::Completed);
        assert_eq!(job.final_method, Some(FinalMethod::ManualReview));
        assert!(job.requires_manual_review);
        assert!(job.manual_review_reason.is_some());
        assert!(job.locked_by.is_none());
    }

    #[test]
    fn retryable_errors_reset_completion_fields_and_requeue() {
        let mut job = ExtractionJob::new("retry-1", "subject", Utc::now(), "hash");
        job.status = QueueStatus::Processing;
        job.final_method = Some(FinalMethod::LlmCompleted);
        job.partial_fields = Some(json!({"k": "v"}));
        job.decision_reason = Some("done".into());
        job.manual_review_reason = Some("review".into());
        job.llm_latency_ms = Some(1200);
        job.completed_at = Some(Utc::now());
        job.locked_by = Some("worker-1".into());

        let retry_after = chrono::Duration::minutes(10);
        let (updated, status) = apply_outcome(
            job,
            Err(JobError::Retryable {
                message: "temporary".into(),
                retry_after: Some(retry_after),
            }),
        );

        assert_eq!(status, QueueStatus::Pending);
        assert_eq!(updated.status, QueueStatus::Pending);
        assert!(updated.final_method.is_none());
        assert!(updated.partial_fields.is_none());
        assert!(updated.decision_reason.is_none());
        assert!(updated.manual_review_reason.is_none());
        assert!(updated.llm_latency_ms.is_none());
        assert!(updated.completed_at.is_none());
        assert!(updated.locked_by.is_none());
        assert!(updated.retry_count > 0);
        assert!(updated.next_retry_at.is_some());
        assert!(
            updated
                .last_error
                .as_ref()
                .map(|e| e.contains("temporary"))
                .unwrap_or(false)
        );
    }

    #[test]
    fn provider_specific_api_keys_fill_default() {
        with_env(
            &[
                ("LLM_PROVIDER", Some("anthropic")),
                ("LLM_API_KEY", None),
                ("ANTHROPIC_API_KEY", Some("anthropic-secret")),
            ],
            || {
                let cfg = LlmRuntimeConfig::from_env();
                assert_eq!(cfg.api_key, "anthropic-secret");
                assert_eq!(cfg.provider, "anthropic");
            },
        );
    }

    #[test]
    fn shadow_defaults_can_use_provider_specific_keys() {
        with_env(
            &[
                ("LLM_PROVIDER", Some("openai")),
                ("LLM_SHADOW_PROVIDER", Some("xai")),
                ("LLM_SHADOW_API_KEY", None),
                ("LLM_API_KEY", Some("openai-main")),
                ("XAI_API_KEY", Some("xai-secret")),
            ],
            || {
                let cfg = LlmRuntimeConfig::from_env();
                assert_eq!(cfg.api_key, "openai-main");
                assert_eq!(cfg.shadow_api_key, "xai-secret");
                assert_eq!(cfg.shadow_provider, "xai");
            },
        );
    }

    #[test]
    fn shadow_key_falls_back_to_primary_when_same_provider() {
        with_env(
            &[
                ("LLM_PROVIDER", Some("openai")),
                ("LLM_SHADOW_PROVIDER", Some("openai")),
                ("LLM_API_KEY", Some("primary-key")),
                ("LLM_SHADOW_API_KEY", None),
            ],
            || {
                let cfg = LlmRuntimeConfig::from_env();
                assert_eq!(cfg.api_key, "primary-key");
                assert_eq!(cfg.shadow_api_key, "primary-key");
            },
        );
    }

    #[test]
    fn shadow_sampling_marks_canary_targets() {
        let mut job = ExtractionJob::new("shadow-msg", "subject", Utc::now(), "hash");
        let cfg = ShadowCompareConfig {
            mode: CompareMode::Shadow,
            sample_percent: 100,
            shadow_provider: "shadow_stub".into(),
            primary_provider: "primary_stub".into(),
        };

        assert!(mark_shadow_canary(&mut job, &cfg));
        assert!(job.canary_target);
    }

    #[test]
    fn no_sampling_when_compare_mode_disabled() {
        let mut job = ExtractionJob::new("shadow-msg-2", "subject", Utc::now(), "hash");
        let cfg = ShadowCompareConfig {
            mode: CompareMode::None,
            sample_percent: 100,
            shadow_provider: "shadow_stub".into(),
            primary_provider: "primary_stub".into(),
        };

        assert!(!mark_shadow_canary(&mut job, &cfg));
        assert!(!job.canary_target);
    }

    #[test]
    fn llm_config_reads_env_overrides() {
        with_env(
            &[
                ("LLM_ENABLED", Some("0")),
                ("LLM_PROVIDER", Some("anthropic")),
                ("LLM_MODEL", Some("claude-3-5-sonnet")),
                ("LLM_ENDPOINT", Some("https://example.com")),
                ("LLM_API_KEY", Some("shadow-key")),
                ("LLM_TIMEOUT_SECONDS", Some("45")),
                ("LLM_MAX_RETRIES", Some("5")),
                ("LLM_RETRY_BACKOFF_SECONDS", Some("7")),
                ("LLM_COMPARE_MODE", Some("shadow")),
                ("LLM_PRIMARY_PROVIDER", Some("anthropic")),
                ("LLM_SHADOW_PROVIDER", Some("openai")),
                ("LLM_SHADOW_SAMPLE_PERCENT", Some("25")),
                ("LLM_SHADOW_API_KEY", Some("shadow-secondary")),
            ],
            || {
                let cfg = LlmRuntimeConfig::from_env();
                let shadow = shadow_config_from_env(&cfg);

                assert!(!cfg.enabled);
                assert_eq!(cfg.provider, "anthropic");
                assert_eq!(cfg.model, "claude-3-5-sonnet");
                assert_eq!(cfg.endpoint, "https://example.com");
                assert_eq!(cfg.api_key, "shadow-key");
                assert_eq!(cfg.timeout_secs, 45);
                assert_eq!(cfg.max_retries, 5);
                assert_eq!(cfg.retry_backoff_secs, 7);
                assert_eq!(cfg.compare_mode, CompareMode::Shadow);
                assert_eq!(cfg.primary_provider, "anthropic");
                assert_eq!(cfg.shadow_provider, "openai");
                assert_eq!(cfg.shadow_sample_percent, 25);
                assert_eq!(cfg.shadow_api_key, "shadow-secondary");
                assert_eq!(shadow.mode, CompareMode::Shadow);
                assert_eq!(shadow.primary_provider, "anthropic");
                assert_eq!(shadow.shadow_provider, "openai");
                assert_eq!(shadow.sample_percent, 25);
            },
        );
    }

    #[test]
    fn llm_provider_defaults_follow_live_endpoints() {
        with_env(
            &[
                ("LLM_PROVIDER", Some("google-genai")),
                ("LLM_MODEL", None),
                ("LLM_ENDPOINT", None),
            ],
            || {
                let cfg = LlmRuntimeConfig::from_env();
                assert_eq!(cfg.model, "gemini-1.5-flash");
                assert_eq!(
                    cfg.endpoint,
                    "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent",
                );
            },
        );

        with_env(
            &[
                ("LLM_PROVIDER", Some("mistral")),
                ("LLM_MODEL", None),
                ("LLM_ENDPOINT", None),
            ],
            || {
                let cfg = LlmRuntimeConfig::from_env();
                assert_eq!(cfg.model, "mistral-large-latest");
                assert_eq!(cfg.endpoint, "https://api.mistral.ai/v1/chat/completions");
            },
        );

        with_env(
            &[
                ("LLM_PROVIDER", Some("huggingface")),
                ("LLM_MODEL", None),
                ("LLM_ENDPOINT", None),
            ],
            || {
                let cfg = LlmRuntimeConfig::from_env();
                assert_eq!(cfg.model, "meta-llama/Meta-Llama-3-70B-Instruct");
                assert_eq!(
                    cfg.endpoint,
                    "https://api-inference.huggingface.co/models/meta-llama/Meta-Llama-3-70B-Instruct",
                );
            },
        );
    }

    #[test]
    fn llm_disabled_routes_to_manual_review() {
        with_env(&[("LLM_ENABLED", Some("0"))], || {
            let llm_config = LlmRuntimeConfig::from_env();
            let mut queue = ExtractionQueue::default();
            let mut job = ExtractionJob::new("disabled", "subject", Utc::now(), "hash");
            job.recommended_method = Some(RecommendedMethod::LlmRecommended);

            queue.enqueue(job);
            queue.process_next_with_worker("sr-llm-worker", |j| {
                handle_llm_job(j, "body", &llm_config)
            });

            let job = &queue.jobs[0];
            assert_eq!(job.status, QueueStatus::Completed);
            assert_eq!(job.final_method, Some(FinalMethod::ManualReview));
            assert!(job.requires_manual_review);
            assert!(
                job.decision_reason
                    .as_ref()
                    .map(|r| r.contains("LLM_DISABLED"))
                    .unwrap_or(false)
            );
        });
    }

    #[test]
    fn missing_api_key_is_manual_review() {
        with_env(&[("LLM_API_KEY", None), ("OPENAI_API_KEY", None)], || {
            let llm_config = LlmRuntimeConfig::from_env();
            let mut queue = ExtractionQueue::default();
            let mut job = ExtractionJob::new("no-key", "subject", Utc::now(), "hash");
            job.recommended_method = Some(RecommendedMethod::LlmRecommended);

            queue.enqueue(job);
            queue.process_next_with_worker("sr-llm-worker", |j| {
                handle_llm_job(j, "body", &llm_config)
            });

            let job = &queue.jobs[0];
            assert_eq!(job.status, QueueStatus::Completed);
            assert_eq!(job.final_method, Some(FinalMethod::ManualReview));
            assert!(job.requires_manual_review);
            let reason = job.decision_reason.as_ref().unwrap();
            assert!(reason.contains("missing LLM_API_KEY"));
        });
    }

    #[tokio::test]
    async fn shadow_compare_executes_request() {
        let server = MockServer::start_async().await;
        let shadow_hit = server
            .mock_async(|when, then| {
                when.method(POST).path("/shadow");
                then.status(200).json_body(json!({
                    "extracted": {"project_name": "shadow"},
                    "latency_ms": 50,
                    "model_used": "shadow-model",
                }));
            })
            .await;

        let mut job = ExtractionJob::new("shadow", "subject", Utc::now(), "hash");
        job.recommended_method = Some(RecommendedMethod::LlmRecommended);
        job.canary_target = true;

        let mut config = LlmRuntimeConfig::from_env();
        config.shadow_endpoint = Some(server.url("/shadow"));
        config.shadow_model = Some("shadow-model".into());
        config.shadow_api_key = "shadow-key".into();

        let shadow_cfg = ShadowCompareConfig {
            mode: CompareMode::Shadow,
            sample_percent: 100,
            shadow_provider: "shadow-provider".into(),
            primary_provider: "primary-provider".into(),
        };

        let handle = spawn_shadow_compare(job, "body".into(), &config, &shadow_cfg)
            .expect("shadow compare spawned");
        handle.await.expect("shadow join ok");

        shadow_hit.assert_async().await;
    }

    #[test]
    fn shadow_compare_skips_without_key() {
        let mut job = ExtractionJob::new("shadow-skip", "subject", Utc::now(), "hash");
        job.recommended_method = Some(RecommendedMethod::LlmRecommended);
        job.canary_target = true;

        let mut config = LlmRuntimeConfig::from_env();
        config.api_key.clear();
        config.shadow_api_key.clear();

        let shadow_cfg = ShadowCompareConfig {
            mode: CompareMode::Shadow,
            sample_percent: 100,
            shadow_provider: "shadow-provider".into(),
            primary_provider: "primary-provider".into(),
        };

        let handle = spawn_shadow_compare(job, "body".into(), &config, &shadow_cfg);
        assert!(handle.is_none());
    }
}
