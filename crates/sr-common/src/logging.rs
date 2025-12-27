use std::panic;
use std::sync::OnceLock;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::writer::BoxMakeWriter;
use tracing_subscriber::EnvFilter;

static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

/// Install a global panic hook that logs panics through `tracing` with file/line
/// context. Safe to call multiple times; the hook is installed once per process.
pub fn install_tracing_panic_hook(app_name: &'static str) {
    static INSTALLED: OnceLock<()> = OnceLock::new();

    INSTALLED.get_or_init(|| {
        let default_hook = panic::take_hook();
        let include_backtrace = std::env::var("SR_LOG_INCLUDE_BACKTRACE")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        panic::set_hook(Box::new(move |info| {
            let thread = std::thread::current();
            let thread_name = thread.name().unwrap_or("unknown");

            let location = info
                .location()
                .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()));
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic payload not string".into());

            tracing::error!(
                application = app_name,
                %thread_name,
                location = location.as_deref().unwrap_or("unknown"),
                panic_message = %message,
                "panic captured"
            );

            if include_backtrace {
                default_hook(info);
            }
        }));
    });
}

fn rotating_file_writer(app_name: &'static str) -> Option<BoxMakeWriter> {
    let dir = std::env::var_os("SR_LOG_DIR")?;
    let dir = std::path::PathBuf::from(dir);
    if let Err(err) = std::fs::create_dir_all(&dir) {
        tracing::warn!(error = %err, "failed to create SR_LOG_DIR; falling back to stdout");
        return None;
    }

    let appender = tracing_appender::rolling::daily(dir, format!("{app_name}.log"));
    let (non_blocking, guard) = tracing_appender::non_blocking(appender);
    let _ = LOG_GUARD.set(guard);
    Some(BoxMakeWriter::new(non_blocking))
}

/// Initialize a tracing subscriber with optional daily log rotation controlled by `SR_LOG_DIR`.
///
/// When `SR_LOG_DIR` is set, logs are written to `<SR_LOG_DIR>/<app>.log` with daily rotation;
/// otherwise, logs continue to be emitted to stdout. Uses `RUST_LOG` for filtering if present.
pub fn init_tracing_subscriber(app_name: &'static str) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let builder = tracing_subscriber::fmt().with_env_filter(env_filter);

    if let Some(writer) = rotating_file_writer(app_name) {
        let _ = builder.with_writer(writer).try_init();
    } else {
        let _ = builder.try_init();
    }
}
