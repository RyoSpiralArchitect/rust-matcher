use chrono::Utc;
use clap::Parser;
use dotenvy::dotenv;
use rand::Rng;
use serde_json::json;
use sr_common::db::{
    MatchResultInsert, create_pool_from_url, insert_match_result, lock_next_pending_job,
    upsert_extraction_job,
};
use sr_common::queue::{
    ExtractionJob, ExtractionQueue, FinalMethod, JobError, JobOutcome, QueueStatus,
    RecommendedMethod,
};
use tokio::time::{sleep, Duration};
use tracing::info;

#[derive(Debug, Clone, PartialEq, Eq)]
enum CompareMode {
    None,
    Shadow,
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
        }
    }
}

impl LlmRuntimeConfig {
    fn from_env() -> Self {
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
                Ok(val) => matches!(val.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"),
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
        let primary_provider = std::env::var("LLM_PRIMARY_PROVIDER")
            .unwrap_or_else(|_| provider.clone());
        let shadow_provider = std::env::var("LLM_SHADOW_PROVIDER").unwrap_or_else(|_| "openai".into());

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

        Self {
            enabled: parse_bool("LLM_ENABLED", true),
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
    let llm_config = LlmRuntimeConfig::default();
    let shadow_config = shadow_config_from_env(&llm_config);

    let mut job = ExtractionJob::new(
        "llm-message-1",
        "llm subject",
        Utc::now(),
        "llm-subject-hash",
    );
    job.recommended_method = Some(RecommendedMethod::LlmRecommended);

    queue.enqueue(job);

    queue.process_next_with_worker(worker_id, |job| handle_llm_job(job, &llm_config));

    if let Some(processed) = queue.jobs.first() {
        if processed.canary_target {
            spawn_shadow_log(processed, &shadow_config);
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

fn mark_shadow_canary(job: &mut ExtractionJob, config: &ShadowCompareConfig) -> bool {
    if config.mode == CompareMode::Shadow && should_sample_shadow(config.sample_percent) {
        job.canary_target = true;
        return true;
    }

    false
}

fn spawn_shadow_log(job: &ExtractionJob, config: &ShadowCompareConfig) {
    if config.mode != CompareMode::Shadow || !job.canary_target {
        return;
    }

    let message_id = job.message_id.clone();
    let shadow_provider = config.shadow_provider.clone();
    let primary_provider = config.primary_provider.clone();

    tokio::spawn(async move {
        info!(
            %message_id,
            shadow_provider,
            primary_provider,
            "shadow comparison sampled; logging for offline diff"
        );
    });
}

fn handle_llm_job(job: &ExtractionJob, config: &LlmRuntimeConfig) -> Result<JobOutcome, JobError> {
    if job.recommended_method == Some(RecommendedMethod::LlmRecommended) {
        if !config.enabled {
            return Err(JobError::Permanent {
                message: "LLM_DISABLED: LLM_ENABLED=0".into(),
            });
        }

        let partial = json!({
            "message_id": job.message_id,
            "llm_provider": config.provider,
            "llm_model": config.model,
            "llm_endpoint": config.endpoint,
        });
        Ok(JobOutcome {
            final_method: FinalMethod::LlmCompleted,
            partial_fields: Some(partial),
            decision_reason: Some(format!(
                "processed by sr-llm-worker via {}",
                config.provider
            )),
            llm_latency_ms: Some(1500),
            requires_manual_review: false,
            manual_review_reason: None,
        })
    } else {
        Err(JobError::Permanent {
            message: "non-llm job routed to sr-llm-worker".into(),
        })
    }
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
    let (processed, _status) = apply_outcome(locked.clone(), handle_llm_job(&locked, llm_config));
    let rows = upsert_extraction_job(pool, &processed).await?;
    info!(rows, message_id = %processed.message_id, "persisted processed job");

    if shadow_selected {
        spawn_shadow_log(&processed, shadow_config);
    }

    // Stubbed persistence of a match result snapshot for the processed job.
    let result = MatchResultInsert {
        talent_id: 1,
        project_id: 1,
        is_knockout: false,
        needs_manual_review: processed.requires_manual_review,
        score_total: Some(0.75),
        score_breakdown: Some(json!({
            "tanka": 0.8,
            "skills": 0.7,
        })),
        engine_version: processed.extractor_version.clone(),
        rule_version: processed.rule_version.clone(),
        ..Default::default()
    };

    let inserted = insert_match_result(pool, &result).await?;
    info!(
        rows = inserted,
        worker_id = %worker_id,
        message_id = %processed.message_id,
        "stub: inserted match_results snapshot"
    );

    Ok(())
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let args = Cli::parse();
    let llm_config = LlmRuntimeConfig::from_env();
    let shadow_config = shadow_config_from_env(&llm_config);
    let pool = create_pool_from_url(&args.db_url)?;
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
        eprintln!("sr-llm-worker failed: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let queue = run_sample_flow();

        assert_eq!(queue.jobs.len(), 1);
        let job = &queue.jobs[0];
        assert_eq!(job.final_method, Some(FinalMethod::LlmCompleted));
        assert_eq!(job.status.as_str(), "completed");
        assert!(!job.requires_manual_review);
        assert!(
            job.decision_reason
                .as_ref()
                .map(|r| r.contains("sr-llm-worker"))
                .unwrap_or(false)
        );
    }

    #[test]
    fn non_llm_jobs_are_sent_to_manual_review() {
        let mut queue = ExtractionQueue::default();
        let mut job = ExtractionJob::new("m2", "subject", Utc::now(), "hash");
        job.recommended_method = Some(RecommendedMethod::RustRecommended);
        let llm_config = LlmRuntimeConfig::default();

        queue.enqueue(job);

        queue.process_next_with_worker("sr-llm-worker", |j| handle_llm_job(j, &llm_config));

        let job = &queue.jobs[0];
        assert_eq!(job.status, QueueStatus::Completed);
        assert_eq!(job.final_method, Some(FinalMethod::ManualReview));
        assert!(job.requires_manual_review);
        assert!(job.manual_review_reason.is_some());
        assert!(job.locked_by.is_none());
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
            queue.process_next_with_worker("sr-llm-worker", |j| handle_llm_job(j, &llm_config));

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
}
