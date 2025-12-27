# Logging & Aggregation Guidance

This project emits structured `tracing` logs to stdout by default, with optional daily file rotation. Use this guide to ship those logs into your aggregation stack and to understand the business metrics surfaced by long‑running workers.

## Shipping application logs

- **Default**: Logs are written to stdout/stderr. Point your collector (Fluent Bit/Vector/Filebeat) at the process stdout stream and include `application`, `thread_name`, `location`, and `panic_message` fields.
- **File rotation**: Set `SR_LOG_DIR=/var/log/rust-matcher` (or any writable path) to rotate daily per binary (`<app>.log`). Configure your collector to tail that directory if you prefer file-based shipping.
- **ELK**: Ship stdout or the rotated files to Logstash/Filebeat. Keep `X-Request-Id` in HTTP paths to correlate requests.
- **Loki**: Send stdout to Promtail. If using file rotation, configure a Loki scrape job against `SR_LOG_DIR`. Labels to keep: `application`, `worker_id`, `message_id`, `request_id`.
- **CloudWatch**: Pipe stdout to the CloudWatch agent/FireLens. When using file rotation, add `SR_LOG_DIR` to `files` in the agent config.

## Controlling panic backtraces

`sr_common::logging::install_tracing_panic_hook` now suppresses Rust's default backtrace in production logs to reduce noise. To enable backtraces when troubleshooting, set:

```
SR_LOG_INCLUDE_BACKTRACE=true
```

This retains structured panic logs while letting the default hook print the full stack trace.

## Business metrics logs (LLM worker)

`sr-llm-worker` now emits periodic operational summaries to `INFO`:

- `jobs_total`: Total jobs processed since start
- `successes`: Completed without manual review
- `permanent_failures`: Completed but require manual review (e.g., fatal errors)
- `retries_scheduled`: Jobs returned to the queue for retry
- `jobs_per_hour`: Throughput based on wall-clock runtime
- `success_rate`: `successes / max(1, jobs_total)` as a float

Frequency is controlled by `SR_LOG_SUMMARY_INTERVAL_SECS` (default: 300). Keep these logs in your aggregation stack to monitor worker health alongside Prometheus metrics.
