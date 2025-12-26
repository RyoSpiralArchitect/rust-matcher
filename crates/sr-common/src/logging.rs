use std::panic;
use std::sync::OnceLock;

/// Install a global panic hook that logs panics through `tracing` with file/line
/// context. Safe to call multiple times; the hook is installed once per process.
pub fn install_tracing_panic_hook(app_name: &'static str) {
    static INSTALLED: OnceLock<()> = OnceLock::new();

    INSTALLED.get_or_init(|| {
        let default_hook = panic::take_hook();

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

            default_hook(info);
        }));
    });
}
