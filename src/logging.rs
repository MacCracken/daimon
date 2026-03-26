//! Logging initialization for the daimon service.

/// Initialize the tracing subscriber.
///
/// Reads `DAIMON_LOG` environment variable for filter directives (defaults to `info`).
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "logging")]
/// daimon::logging::try_init();
/// ```
pub fn try_init() {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_env("DAIMON_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .try_init()
        .ok();
}
