use chrono::Utc;
use clap::Parser;
use dotenvy::dotenv;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sr_common::db::util::TimedClientExt;
use sr_common::db::{
    create_pool_from_url_checked, fetch_email_body, lock_next_pending_job, run_migrations,
    upsert_extraction_job, PgPool,
};
use sr_common::logging::{init_tracing_subscriber, install_tracing_panic_hook};
use sr_common::queue::{
    ExtractionJob, ExtractionQueue, FinalMethod, JobError, JobOutcome, QueueStatus,
    RecommendedMethod,
};
use sr_metrics::init_metrics;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration as StdDuration, Instant};
use tokio::sync::Semaphore;
use tokio::time::{sleep, Duration};
use tracing::{error, info, info_span, warn, Instrument, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
enum CompareMode {
    None,
    Shadow,
}

const MAX_RETRY_COUNT: u32 = 100;

fn build_http_client(timeout_secs: u64) -> Result<Client, reqwest::Error> {
    Client::builder()
        .no_proxy()
        .timeout(StdDuration::from_secs(timeout_secs))
        .build()
}

fn current_trace_id() -> Option<String> {
    Span::current().id().map(|id| format!("{id:?}"))
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
            enabled: false,
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

        fn parse_timeout_secs(key: &str, default: u64) -> u64 {
            const MAX_TIMEOUT_SECS: u64 = 600;

            match std::env::var(key) {
                Ok(raw) => match raw.parse::<u64>() {
                    Ok(value) if value == 0 => {
                        warn!(
                            key,
                            default, "timeout must be greater than 0 seconds; using default"
                        );
                        default
                    }
                    Ok(value) if value > MAX_TIMEOUT_SECS => {
                        warn!(
                            key,
                            requested = value,
                            capped = MAX_TIMEOUT_SECS,
                            "timeout exceeds safety cap; capping value"
                        );
                        MAX_TIMEOUT_SECS
                    }
                    Ok(value) => value,
                    Err(_) => {
                        warn!(key, default, "invalid timeout value; using default");
                        default
                    }
                },
                Err(_) => default,
            }
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

        let enabled = parse_bool("LLM_ENABLED", false);
        if enabled && api_key.is_empty() {
            warn!(
                provider = %provider,
                "LLM_API_KEY is empty while LLM_ENABLED=true; requests will fall back to manual review"
            );
        }

        Self {
            enabled,
            provider: provider.clone(),
            model: std::env::var("LLM_MODEL").unwrap_or_else(|_| default_model),
            endpoint: std::env::var("LLM_ENDPOINT").unwrap_or_else(|_| default_endpoint),
            api_key,
            timeout_secs: parse_timeout_secs("LLM_TIMEOUT_SECONDS", 30),
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
    max_in_flight: usize,
}

#[derive(Clone)]
struct ShadowCompareRuntime {
    config: ShadowCompareConfig,
    in_flight: Arc<Semaphore>,
    tasks: Arc<tokio::sync::Mutex<tokio::task::JoinSet<()>>>,
}

impl ShadowCompareRuntime {
    fn new(config: ShadowCompareConfig) -> Self {
        let max_in_flight = config.max_in_flight.max(1);
        Self {
            in_flight: Arc::new(Semaphore::new(max_in_flight)),
            config,
            tasks: Arc::new(tokio::sync::Mutex::new(tokio::task::JoinSet::new())),
        }
    }

    fn config(&self) -> &ShadowCompareConfig {
        &self.config
    }

    async fn track_shadow_task<F>(&self, task: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut tasks = self.tasks.lock().await;
        tasks.spawn(task);
        Self::drain_finished_tasks(&mut tasks);
    }

    async fn wait_for_all(&self) {
        let mut tasks = self.tasks.lock().await;
        while let Some(result) = tasks.join_next().await {
            if let Err(err) = result {
                warn!(error = %err, "shadow comparison task failed");
            }
        }
    }

    fn drain_finished_tasks(tasks: &mut tokio::task::JoinSet<()>) {
        while let Some(result) = tasks.try_join_next() {
            if let Err(err) = result {
                warn!(error = %err, "shadow comparison task failed");
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct LlmRequest {
    message_id: String,
    source_text: String,
    extractor_hints: serde_json::Value,
    model: String,
    timeout_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, Default, Serialize)]
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

    /// Minimum delay in milliseconds between finishing a job and locking the next one
    #[arg(long, env = "LLM_JOB_THROTTLE_MS", default_value_t = 100)]
    min_job_gap_ms: u64,
}

pub fn run_sample_flow_with_worker(worker_id: &str) -> ExtractionQueue {
    let mut queue = ExtractionQueue::default();
    let llm_config = LlmRuntimeConfig::from_env();
    let shadow_config = shadow_config_from_env(&llm_config);
    let shadow_runtime = ShadowCompareRuntime::new(shadow_config);
    let client = build_http_client(llm_config.timeout_secs)
        .expect("failed to build http client for sample flow");
    let rt = tokio::runtime::Runtime::new().expect("failed to build runtime for sample flow");

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
        rt.block_on(handle_llm_job(
            job,
            sample_body,
            &llm_config,
            &client,
            worker_id,
        ))
    });

    if let Some(processed) = queue.jobs.first() {
        if processed.canary_target {
            rt.block_on(spawn_shadow_compare(
                processed.clone(),
                sample_body.to_string(),
                &llm_config,
                &shadow_runtime,
                &client,
                worker_id,
                None,
            ));
            rt.block_on(shadow_runtime.wait_for_all());
        }
    }

    queue
}

pub fn run_sample_flow() -> ExtractionQueue {
    run_sample_flow_with_worker("sr-llm-worker")
}

fn shadow_config_from_env(config: &LlmRuntimeConfig) -> ShadowCompareConfig {
    fn parse_shadow_limit() -> usize {
        std::env::var("LLM_SHADOW_MAX_IN_FLIGHT")
            .ok()
            .and_then(|raw| raw.parse::<usize>().ok())
            .filter(|limit| *limit > 0)
            .unwrap_or(5)
    }

    ShadowCompareConfig {
        mode: config.compare_mode.clone(),
        sample_percent: config.shadow_sample_percent,
        shadow_provider: config.shadow_provider.clone(),
        primary_provider: config.primary_provider.clone(),
        max_in_flight: parse_shadow_limit(),
    }
}

fn should_sample_shadow(sample_percent: u8, salt: &str) -> bool {
    if sample_percent == 0 {
        return false;
    }

    let normalized_percent = sample_percent.min(100);
    let mut hasher = DefaultHasher::new();
    salt.hash(&mut hasher);
    let bucket = (hasher.finish() % 100) as u8;
    bucket < normalized_percent
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

async fn perform_llm_request(
    client: &Client,
    config: &LlmRuntimeConfig,
    endpoint: &str,
    api_key: &str,
    request: &LlmRequest,
    request_id: &str,
    trace_id: Option<String>,
) -> Result<LlmResponse, JobError> {
    let provider = config.provider.clone();
    let model = request.model.clone();
    let span = info_span!(
        "perform_llm_request",
        %request_id,
        %endpoint,
        %provider,
        %model
    );
    let _entered = span.enter();
    let start = Instant::now();

    for attempt in 0..=config.max_retries {
        let mut request_builder = client
            .post(endpoint)
            .bearer_auth(api_key)
            .header("x-request-id", request_id)
            .json(request);

        if let Some(trace_id) = trace_id.clone().or_else(current_trace_id) {
            request_builder = request_builder
                .header("x-trace-id", &trace_id)
                .header("traceparent", trace_id);
        }

        let response = request_builder.send().await;

        match response {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
                    metrics::histogram!(
                        "llm_request_latency_ms",
                        "provider" => provider.clone(),
                        "model" => model.clone(),
                        "outcome" => "success"
                    )
                    .record(latency_ms);
                    metrics::counter!(
                        "llm_request_total",
                        "provider" => provider.clone(),
                        "model" => model.clone(),
                        "outcome" => "success"
                    )
                    .increment(1);
                    return resp
                        .json::<LlmResponse>()
                        .await
                        .map_err(|err| JobError::Permanent {
                            message: format!("invalid llm response body: {err}"),
                        });
                }

                if is_retryable_status(status) && attempt < config.max_retries {
                    metrics::counter!(
                        "llm_request_total",
                        "provider" => provider.clone(),
                        "model" => model.clone(),
                        "outcome" => "retry"
                    )
                    .increment(1);
                    sleep(Duration::from_secs(config.retry_backoff_secs)).await;
                    continue;
                }

                let body = resp.text().await.unwrap_or_default();
                let message = format!("llm call failed with status {status}: {body}");
                metrics::counter!(
                    "llm_request_total",
                    "provider" => provider.clone(),
                    "model" => model.clone(),
                    "outcome" => if is_retryable_status(status) { "retryable_failure" } else { "failure" }
                )
                .increment(1);
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
                    metrics::counter!(
                        "llm_request_total",
                        "provider" => provider.clone(),
                        "model" => model.clone(),
                        "outcome" => "retry"
                    )
                    .increment(1);
                    sleep(Duration::from_secs(config.retry_backoff_secs)).await;
                    continue;
                }

                metrics::counter!(
                    "llm_request_total",
                    "provider" => provider.clone(),
                    "model" => model.clone(),
                    "outcome" => "failure"
                )
                .increment(1);

                return Err(JobError::Retryable {
                    message: format!("llm request error: {err}"),
                    retry_after: Some(chrono::Duration::seconds(config.retry_backoff_secs as i64)),
                });
            }
        }
    }

    metrics::counter!(
        "llm_request_total",
        "provider" => provider,
        "model" => model,
        "outcome" => "exhausted"
    )
    .increment(1);

    Err(JobError::Retryable {
        message: "llm retries exhausted".into(),
        retry_after: Some(chrono::Duration::seconds(config.retry_backoff_secs as i64)),
    })
}

fn mark_shadow_canary(job: &mut ExtractionJob, config: &ShadowCompareConfig) -> bool {
    let sampling_key = if job.message_id.is_empty() {
        job.subject_hash.as_str()
    } else {
        job.message_id.as_str()
    };

    if config.mode == CompareMode::Shadow
        && should_sample_shadow(config.sample_percent, sampling_key)
    {
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

#[derive(Debug)]
struct ShadowComparisonRecord {
    message_id: String,
    primary_provider: String,
    shadow_provider: String,
    primary_response: Value,
    shadow_response: Option<Value>,
    primary_latency_ms: Option<i32>,
    shadow_latency_ms: Option<i32>,
    diff_summary: Value,
}

async fn persist_shadow_comparison(
    pool: PgPool,
    record: ShadowComparisonRecord,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(
            "INSERT INTO ses.llm_comparison_results (
                message_id,
                primary_provider,
                shadow_provider,
                primary_response,
                shadow_response,
                primary_latency_ms,
                shadow_latency_ms,
                diff_summary
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8
            )",
        )
        .await?;

    client
        .timed_execute(
            &stmt,
            &[
                &record.message_id,
                &record.primary_provider,
                &record.shadow_provider,
                &record.primary_response,
                &record.shadow_response,
                &record.primary_latency_ms,
                &record.shadow_latency_ms,
                &record.diff_summary,
            ],
            "insert_llm_comparison_result",
        )
        .await
        .map_err(|err| Box::new(err) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(())
}

async fn spawn_shadow_compare(
    job: ExtractionJob,
    body_text: String,
    config: &LlmRuntimeConfig,
    shadow: &ShadowCompareRuntime,
    client: &Client,
    worker_id: &str,
    pool: Option<PgPool>,
) -> bool {
    let shadow_config = shadow.config();

    if shadow_config.mode != CompareMode::Shadow || !job.canary_target {
        return false;
    }

    let permit = match shadow.in_flight.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            info!(
                message_id = %job.message_id,
                max_in_flight = shadow_config.max_in_flight,
                "skipping shadow comparison because in-flight limit has been reached",
            );
            return false;
        }
    };

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
        return false;
    }
    let shadow_provider = shadow_config.shadow_provider.clone();
    let primary_provider = shadow_config.primary_provider.clone();
    let mut shadow_config = config.clone();
    shadow_config.model = shadow_model;
    let trace_id = current_trace_id();
    let worker_label = worker_id.to_string();
    let job_id = job.id;
    let message_id = job.message_id.clone();
    let client = client.clone();
    let pool = pool.clone();
    let shadow_span = info_span!(
        "shadow_compare",
        worker_id = %worker_label,
        message_id = %message_id,
        job_id = job_id
    );

    shadow
        .track_shadow_task(
            async move {
                let _permit = permit;
                let request = build_llm_request(&job, &body_text, &shadow_config);
                let primary_final_method = job.final_method.clone();
                let primary_decision = job.decision_reason.clone();
                let primary_requires_review = job.requires_manual_review;
                let primary_latency_ms = job.llm_latency_ms;
                match perform_llm_request(
                    &client,
                    &shadow_config,
                    &shadow_endpoint,
                    &shadow_api_key,
                    &request,
                    &message_id,
                    trace_id.clone(),
                )
                .await
                {
                    Ok(shadow_resp) => {
                        let primary_fields = job.partial_fields.clone().unwrap_or_default();
                        let diff = if shadow_resp.extracted == primary_fields {
                            "match"
                        } else {
                            "diff"
                        };
                        info!(
                            worker_id = %worker_label,
                            message_id = %message_id,
                            job_id = job_id,
                            %shadow_provider,
                            %primary_provider,
                            diff,
                            shadow_model_used = shadow_resp.model_used,
                            shadow_latency_ms = shadow_resp.latency_ms,
                            "shadow comparison completed",
                        );

                        if let Some(pool) = pool.clone() {
                            let record = ShadowComparisonRecord {
                                message_id: message_id.clone(),
                                primary_provider: primary_provider.clone(),
                                shadow_provider: shadow_provider.clone(),
                                primary_response: json!({
                                    "extracted": primary_fields,
                                    "decision_reason": primary_decision,
                                    "final_method": primary_final_method.as_ref().map(|m| m.as_str()),
                                    "requires_manual_review": primary_requires_review,
                                }),
                                shadow_response: serde_json::to_value(&shadow_resp).ok(),
                                primary_latency_ms,
                                shadow_latency_ms: shadow_resp.latency_ms,
                                diff_summary: json!({
                                    "match": diff == "match",
                                    "diff": diff,
                                    "missing_fields": shadow_resp.missing_fields,
                                }),
                            };

                            if let Err(err) = persist_shadow_comparison(pool, record).await {
                                warn!(
                                    worker_id = %worker_label,
                                    message_id = %message_id,
                                    error = %err,
                                    "failed to persist shadow comparison"
                                );
                            }
                        }
                    }
                    Err(err) => {
                        let err_message = match err {
                            JobError::Retryable { ref message, .. } => message.clone(),
                            JobError::Permanent { ref message } => message.clone(),
                        };
                        info!(
                            worker_id = %worker_label,
                            message_id = %message_id,
                            job_id = job_id,
                            %shadow_provider,
                            %primary_provider,
                            error = %err_message,
                            "shadow comparison failed",
                        );

                        if let Some(pool) = pool.clone() {
                            let record = ShadowComparisonRecord {
                                message_id: message_id.clone(),
                                primary_provider: primary_provider.clone(),
                                shadow_provider: shadow_provider.clone(),
                                primary_response: json!({
                                    "extracted": job.partial_fields.clone().unwrap_or_default(),
                                    "decision_reason": primary_decision,
                                    "final_method": primary_final_method.as_ref().map(|m| m.as_str()),
                                    "requires_manual_review": primary_requires_review,
                                }),
                                shadow_response: None,
                                primary_latency_ms,
                                shadow_latency_ms: None,
                                diff_summary: json!({
                                    "error": err_message,
                                }),
                            };

                            if let Err(err) = persist_shadow_comparison(pool, record).await {
                                warn!(
                                    worker_id = %worker_label,
                                    message_id = %message_id,
                                    error = %err,
                                    "failed to persist failed shadow comparison"
                                );
                            }
                        }
                    }
                }
            }
            .instrument(shadow_span),
        )
        .await;

    true
}

async fn handle_llm_job(
    job: &ExtractionJob,
    body_text: &str,
    config: &LlmRuntimeConfig,
    client: &Client,
    worker_id: &str,
) -> Result<JobOutcome, JobError> {
    let _span_guard = info_span!(
        "llm_job",
        worker_id = %worker_id,
        message_id = %job.message_id,
        job_id = job.id
    )
    .entered();
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
    let response = perform_llm_request(
        client,
        config,
        &config.endpoint,
        &config.api_key,
        &request,
        &job.message_id,
        current_trace_id(),
    )
    .await?;
    let latency = response
        .latency_ms
        .or_else(|| (Utc::now() - started).num_milliseconds().try_into().ok());

    let mut requires_manual_review = response.requires_manual_review;
    let mut extracted_fields = Some(response.extracted.clone());
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
        extracted_fields = None;
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
        partial_fields: extracted_fields,
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
            let next_retry_count = job.retry_count.saturating_add(1);
            if next_retry_count > MAX_RETRY_COUNT {
                job.status = QueueStatus::Completed;
                job.retry_count = next_retry_count;
                job.final_method = Some(FinalMethod::ManualReview);
                job.last_error = Some(format!(
                    "retry limit exceeded after {next_retry_count} attempts: {message}"
                ));
                job.decision_reason = job.last_error.clone();
                job.manual_review_reason = job.last_error.clone();
                job.requires_manual_review = true;
                job.next_retry_at = None;
                job.partial_fields = None;
                job.llm_latency_ms = None;
                job.completed_at = Some(finished_at);
                job.updated_at = finished_at;
                job.processing_started_at = None;
                job.locked_by = None;
            } else {
                job.status = QueueStatus::Pending;
                job.retry_count = next_retry_count;
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
                job.requires_manual_review = false;
                job.processing_started_at = None;
                job.locked_by = None;
            }
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
    client: &Client,
    shadow_config: &ShadowCompareRuntime,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        worker_id = %worker_id,
        message_id = %locked.message_id,
        job_id = locked.id,
        "starting locked job processing"
    );

    let shadow_selected = mark_shadow_canary(&mut locked, shadow_config.config());
    let body_text = match fetch_email_body(pool, &locked.message_id).await {
        Ok(Some(body)) => body,
        Ok(None) => {
            warn!(
                worker_id = %worker_id,
                message_id = %locked.message_id,
                job_id = locked.id,
                "missing source_text in anken_emails; skipping job"
            );
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
            warn!(
                worker_id = %worker_id,
                message_id = %locked.message_id,
                job_id = locked.id,
                error = %err,
                "failed to fetch email body"
            );
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
    if body_text.trim().is_empty() {
        warn!(
            worker_id = %worker_id,
            message_id = %locked.message_id,
            job_id = locked.id,
            "empty email body text; forcing manual review"
        );
        let (processed, _) = apply_outcome(
            locked.clone(),
            Err(JobError::Permanent {
                message: "missing_body_text".into(),
            }),
        );
        upsert_extraction_job(pool, &processed).await?;
        return Ok(());
    }

    let outcome = handle_llm_job(&locked, &body_text, llm_config, client, worker_id).await;
    if let Err(err) = &outcome {
        let err_message = match err {
            JobError::Retryable { message, .. } => message.as_str(),
            JobError::Permanent { message } => message.as_str(),
        };
        warn!(
            worker_id = %worker_id,
            message_id = %locked.message_id,
            job_id = locked.id,
            error = %err_message,
            "llm job finished with error"
        );
    } else {
        info!(
            worker_id = %worker_id,
            message_id = %locked.message_id,
            job_id = locked.id,
            "llm job finished successfully"
        );
    }

    let (processed, status) = apply_outcome(locked.clone(), outcome);
    let rows = upsert_extraction_job(pool, &processed).await?;
    let worker_label = worker_id.to_string();
    metrics::counter!(
        "llm_jobs_completed_total",
        "worker_id" => worker_label.clone(),
        "status" => status.as_str().to_string()
    )
    .increment(1);
    info!(
        rows,
        worker_id = %worker_id,
        message_id = %processed.message_id,
        job_id = processed.id,
        status = %status.as_str(),
        "persisted processed job",
    );

    if shadow_selected {
        metrics::counter!(
            "shadow_comparisons_spawned_total",
            "worker_id" => worker_label
        )
        .increment(1);
        spawn_shadow_compare(
            processed.clone(),
            body_text,
            llm_config,
            shadow_config,
            client,
            worker_id,
            Some(pool.clone()),
        )
        .await;
    }

    info!(
        worker_id = %worker_id,
        message_id = %processed.message_id,
        job_id = processed.id,
        status = %processed.status.as_str(),
        "finished locked job processing"
    );

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut sigterm) = signal(SignalKind::terminate()) {
            let _ = sigterm.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    init_tracing_subscriber(env!("CARGO_PKG_NAME"));
    install_tracing_panic_hook(env!("CARGO_PKG_NAME"));
    init_metrics("METRICS_PORT", 9898);

    let args = Cli::parse();
    let llm_config = LlmRuntimeConfig::from_env();
    let shadow_config = shadow_config_from_env(&llm_config);
    let shadow_runtime = ShadowCompareRuntime::new(shadow_config);
    let llm_client = build_http_client(llm_config.timeout_secs)
        .map_err(|err| -> Box<dyn std::error::Error> { Box::new(err) })?;

    // Health check LLM endpoint before entering the work loop to fail fast.
    if llm_config.enabled {
        let client = build_http_client(llm_config.timeout_secs.min(10)).map_err(|err| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("failed to build http client for health check: {err}"),
            )
        })?;
        let response = client
            .get(&llm_config.endpoint)
            .header("x-request-id", "startup-health-check")
            .send()
            .await
            .map_err(|err| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("LLM endpoint unreachable at startup: {err}"),
                )
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("LLM endpoint health check failed with status {status}: {body}"),
            )
            .into());
        }
    }

    if shadow_runtime.config().mode == CompareMode::Shadow {
        if llm_config.shadow_api_key.is_empty() && llm_config.api_key.is_empty() {
            warn!(
                shadow_provider = %shadow_runtime.config().shadow_provider,
                "LLM_COMPARE_MODE=shadow set but no LLM_SHADOW_API_KEY or LLM_API_KEY configured; shadow requests will be skipped"
            );
        }
    }

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
        shadow_mode = ?shadow_runtime.config().mode,
        shadow_sample_percent = shadow_runtime.config().sample_percent,
        shadow_max_in_flight = shadow_runtime.config().max_in_flight,
        "created postgres connection pool for llm worker",
    );

    let mut processed_jobs = 0usize;
    let max_jobs = args.max_jobs.unwrap_or(usize::MAX);
    let worker_label = args.worker_id.clone();
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);

    'work: while processed_jobs < max_jobs {
        let maybe_job = tokio::select! {
            res = lock_next_pending_job(&pool, &args.worker_id, Utc::now()) => res?,
            _ = &mut shutdown => {
                info!("shutdown signal received; draining in-flight work");
                break;
            }
        };

        let Some(job) = maybe_job else {
            if args.exit_on_empty {
                if processed_jobs == 0 {
                    info!("no pending jobs found; exiting");
                }
                break;
            }

            let sleep_duration = Duration::from_millis(args.idle_poll_interval_ms);
            tokio::select! {
                _ = &mut shutdown => {
                    info!("shutdown signal received during idle wait; stopping work loop");
                    break 'work;
                }
                _ = sleep(sleep_duration) => {}
            }
            continue;
        };

        let job_span = info_span!(
            "process_job",
            job_id = job.id,
            message_id = %job.message_id,
            worker_id = %args.worker_id,
            request_id = %job.message_id
        );
        let _entered = job_span.enter();
        metrics::counter!(
            "llm_jobs_started_total",
            "worker_id" => worker_label.clone()
        )
        .increment(1);
        metrics::gauge!("llm_jobs_inflight", "worker_id" => worker_label.clone()).set(1.0);

        process_locked_job(
            &pool,
            &args.worker_id,
            job,
            &llm_config,
            &llm_client,
            &shadow_runtime,
        )
        .await?;
        processed_jobs += 1;
        metrics::gauge!("llm_jobs_inflight", "worker_id" => worker_label.clone()).set(0.0);

        if args.min_job_gap_ms > 0 {
            let throttle = Duration::from_millis(args.min_job_gap_ms);
            tokio::select! {
                _ = &mut shutdown => {
                    info!("shutdown signal received during throttle wait; stopping work loop");
                    break 'work;
                }
                _ = sleep(throttle) => {}
            }
        }
    }

    shadow_runtime.wait_for_all().await;

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
    use mockito::Server;
    use serial_test::serial;
    use std::{panic, sync::Mutex};

    fn with_env(vars: &[(&str, Option<&str>)], f: impl FnOnce()) {
        static ENV_GUARD: Mutex<()> = Mutex::new(());
        let _guard = ENV_GUARD.lock().unwrap();

        let mut vars = vars.to_vec();
        if !vars.iter().any(|(key, _)| *key == "LLM_ENABLED") {
            vars.push(("LLM_ENABLED", Some("1")));
        }

        let prev: Vec<(String, Option<String>)> = vars
            .iter()
            .map(|(key, value)| {
                let previous = std::env::var(key).ok();
                match value {
                    Some(v) => std::env::set_var(key, v),
                    None => std::env::remove_var(key),
                }
                (key.to_string(), previous)
            })
            .collect();

        let result = panic::catch_unwind(panic::AssertUnwindSafe(f));

        for (key, previous) in prev {
            if let Some(v) = previous {
                std::env::set_var(&key, v);
            } else {
                std::env::remove_var(&key);
            }
        }

        if let Err(panic) = result {
            panic::resume_unwind(panic);
        }
    }

    fn mock_llm_extract(server: &mut Server, body: serde_json::Value) -> mockito::Mock {
        server
            .mock("POST", "/api/v1/extract")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body.to_string())
            .create()
    }

    #[test]
    #[serial]
    fn llm_job_is_marked_completed() {
        let mut server = Server::new();
        let mock = mock_llm_extract(
            &mut server,
            json!({
                "extracted": {"project_name": "from-llm"},
                "latency_ms": 222,
                "reason": "llm ok",
            }),
        );

        let endpoint = format!("{}/api/v1/extract", server.url());
        with_env(
            &[
                ("LLM_ENDPOINT", Some(&endpoint)),
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
                assert!(job
                    .decision_reason
                    .as_ref()
                    .map(|r| r.contains("llm ok"))
                    .unwrap_or(false));
            },
        );
        mock.assert();
    }

    #[test]
    #[serial]
    fn llm_missing_fields_trigger_manual_review() {
        let mut server = Server::new();
        let mock = mock_llm_extract(
            &mut server,
            json!({
                "message_id": "llm-message-1",
                "extracted": {"project_name": "from-llm"},
                "latency_ms": 120,
                "reason": "partial extraction",
                "missing_fields": ["skills", "years_of_experience"],
                "status": "partial",
            }),
        );

        let endpoint = format!("{}/api/v1/extract", server.url());
        with_env(
            &[
                ("LLM_ENDPOINT", Some(&endpoint)),
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
        mock.assert();
    }

    #[test]
    #[serial]
    fn llm_message_id_mismatch_forces_manual_review() {
        let mut server = Server::new();
        let mock = mock_llm_extract(
            &mut server,
            json!({
                "message_id": "unexpected",
                "extracted": {"project_name": "from-llm"},
                "latency_ms": 80,
                "status": "ok",
            }),
        );

        let endpoint = format!("{}/api/v1/extract", server.url());
        with_env(
            &[
                ("LLM_ENDPOINT", Some(&endpoint)),
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
        mock.assert();
    }

    #[test]
    fn non_llm_jobs_are_sent_to_manual_review() {
        let mut queue = ExtractionQueue::default();
        let mut job = ExtractionJob::new("m2", "subject", Utc::now(), "hash");
        job.recommended_method = Some(RecommendedMethod::RustRecommended);
        let llm_config = LlmRuntimeConfig::default();
        let client = build_http_client(llm_config.timeout_secs).unwrap();

        queue.enqueue(job);

        queue.process_next_with_worker("sr-llm-worker", |j| {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(handle_llm_job(
                    j,
                    "body",
                    &llm_config,
                    &client,
                    "sr-llm-worker",
                ))
        });

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
        assert!(updated
            .last_error
            .as_ref()
            .map(|e| e.contains("temporary"))
            .unwrap_or(false));
    }

    #[test]
    #[serial]
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
    #[serial]
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
    #[serial]
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
            max_in_flight: 5,
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
            max_in_flight: 5,
        };

        assert!(!mark_shadow_canary(&mut job, &cfg));
        assert!(!job.canary_target);
    }

    #[test]
    fn shadow_sampling_is_deterministic() {
        assert_eq!(
            should_sample_shadow(25, "shadow-key"),
            should_sample_shadow(25, "shadow-key")
        );

        let sampled = (0..1000)
            .filter(|i| should_sample_shadow(10, &format!("job-{i}")))
            .count();

        assert!(sampled >= 70 && sampled <= 130);
    }

    #[test]
    #[serial]
    fn llm_timeout_rejects_zero() {
        with_env(&[("LLM_TIMEOUT_SECONDS", Some("0"))], || {
            let cfg = LlmRuntimeConfig::from_env();
            assert_eq!(cfg.timeout_secs, 30);
        });
    }

    #[test]
    #[serial]
    fn llm_timeout_caps_upper_bound() {
        with_env(&[("LLM_TIMEOUT_SECONDS", Some("3600"))], || {
            let cfg = LlmRuntimeConfig::from_env();
            assert_eq!(cfg.timeout_secs, 600);
        });
    }

    #[test]
    #[serial]
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
                assert_eq!(shadow.max_in_flight, 5);
            },
        );
    }

    #[test]
    #[serial]
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
    #[serial]
    fn llm_disabled_routes_to_manual_review() {
        with_env(&[("LLM_ENABLED", Some("0"))], || {
            let llm_config = LlmRuntimeConfig::from_env();
            let client = build_http_client(llm_config.timeout_secs).unwrap();
            let mut queue = ExtractionQueue::default();
            let mut job = ExtractionJob::new("disabled", "subject", Utc::now(), "hash");
            job.recommended_method = Some(RecommendedMethod::LlmRecommended);

            queue.enqueue(job);
            queue.process_next_with_worker("sr-llm-worker", |j| {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(handle_llm_job(
                        j,
                        "body",
                        &llm_config,
                        &client,
                        "sr-llm-worker",
                    ))
            });

            let job = &queue.jobs[0];
            assert_eq!(job.status, QueueStatus::Completed);
            assert_eq!(job.final_method, Some(FinalMethod::ManualReview));
            assert!(job.requires_manual_review);
            assert!(job
                .decision_reason
                .as_ref()
                .map(|r| r.contains("LLM_DISABLED"))
                .unwrap_or(false));
        });
    }

    #[test]
    #[serial]
    fn missing_api_key_is_manual_review() {
        with_env(&[("LLM_API_KEY", None), ("OPENAI_API_KEY", None)], || {
            let llm_config = LlmRuntimeConfig::from_env();
            let client = build_http_client(llm_config.timeout_secs).unwrap();
            let mut queue = ExtractionQueue::default();
            let mut job = ExtractionJob::new("no-key", "subject", Utc::now(), "hash");
            job.recommended_method = Some(RecommendedMethod::LlmRecommended);

            queue.enqueue(job);
            queue.process_next_with_worker("sr-llm-worker", |j| {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(handle_llm_job(
                        j,
                        "body",
                        &llm_config,
                        &client,
                        "sr-llm-worker",
                    ))
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
    #[serial]
    async fn shadow_compare_executes_request() {
        let mut server = Server::new_async().await;
        let shadow_hit = server
            .mock("POST", "/shadow")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "extracted": {"project_name": "shadow"},
                    "latency_ms": 50,
                    "model_used": "shadow-model",
                })
                .to_string(),
            )
            .create();

        let mut job = ExtractionJob::new("shadow", "subject", Utc::now(), "hash");
        job.recommended_method = Some(RecommendedMethod::LlmRecommended);
        job.canary_target = true;

        let mut config = LlmRuntimeConfig::from_env();
        config.shadow_endpoint = Some(format!("{}/shadow", server.url()));
        config.shadow_model = Some("shadow-model".into());
        config.shadow_api_key = "shadow-key".into();
        let client = build_http_client(config.timeout_secs).unwrap();

        let shadow_cfg = ShadowCompareConfig {
            mode: CompareMode::Shadow,
            sample_percent: 100,
            shadow_provider: "shadow-provider".into(),
            primary_provider: "primary-provider".into(),
            max_in_flight: 2,
        };

        let shadow_runtime = ShadowCompareRuntime::new(shadow_cfg);

        let spawned = spawn_shadow_compare(
            job,
            "body".into(),
            &config,
            &shadow_runtime,
            &client,
            "test-worker",
            None,
        )
        .await;
        assert!(spawned, "shadow compare should spawn");
        shadow_runtime.wait_for_all().await;

        shadow_hit.assert();
    }

    #[tokio::test]
    #[serial]
    async fn shadow_compare_skips_without_key() {
        let mut job = ExtractionJob::new("shadow-skip", "subject", Utc::now(), "hash");
        job.recommended_method = Some(RecommendedMethod::LlmRecommended);
        job.canary_target = true;

        let mut config = LlmRuntimeConfig::from_env();
        config.api_key.clear();
        config.shadow_api_key.clear();
        let client = build_http_client(config.timeout_secs).unwrap();

        let shadow_cfg = ShadowCompareConfig {
            mode: CompareMode::Shadow,
            sample_percent: 100,
            shadow_provider: "shadow-provider".into(),
            primary_provider: "primary-provider".into(),
            max_in_flight: 2,
        };

        let shadow_runtime = ShadowCompareRuntime::new(shadow_cfg);

        let spawned = spawn_shadow_compare(
            job,
            "body".into(),
            &config,
            &shadow_runtime,
            &client,
            "test-worker",
            None,
        )
        .await;
        assert!(!spawned);
    }

    #[tokio::test]
    #[serial]
    async fn shadow_compare_respects_in_flight_limit() {
        let mut server = Server::new_async().await;
        let _shadow_hit = server
            .mock("POST", "/shadow")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "extracted": {"project_name": "shadow"},
                    "latency_ms": 50,
                    "model_used": "shadow-model",
                })
                .to_string(),
            )
            .create();

        let mut job1 = ExtractionJob::new("shadow-1", "subject", Utc::now(), "hash");
        job1.recommended_method = Some(RecommendedMethod::LlmRecommended);
        job1.canary_target = true;

        let mut job2 = ExtractionJob::new("shadow-2", "subject", Utc::now(), "hash");
        job2.recommended_method = Some(RecommendedMethod::LlmRecommended);
        job2.canary_target = true;

        let mut config = LlmRuntimeConfig::from_env();
        config.shadow_endpoint = Some(format!("{}/shadow", server.url()));
        config.shadow_model = Some("shadow-model".into());
        config.shadow_api_key = "shadow-key".into();
        let client = build_http_client(config.timeout_secs).unwrap();

        let shadow_cfg = ShadowCompareConfig {
            mode: CompareMode::Shadow,
            sample_percent: 100,
            shadow_provider: "shadow-provider".into(),
            primary_provider: "primary-provider".into(),
            max_in_flight: 1,
        };

        let shadow_runtime = ShadowCompareRuntime::new(shadow_cfg);

        let spawned_first = spawn_shadow_compare(
            job1,
            "body".into(),
            &config,
            &shadow_runtime,
            &client,
            "test-worker",
            None,
        )
        .await;
        assert!(spawned_first, "first shadow compare should spawn");
        let spawned_second = spawn_shadow_compare(
            job2,
            "body".into(),
            &config,
            &shadow_runtime,
            &client,
            "test-worker",
            None,
        )
        .await;
        assert!(
            !spawned_second,
            "second shadow compare should be limited by in-flight semaphore"
        );

        shadow_runtime.wait_for_all().await;
    }
}
