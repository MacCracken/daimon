//! Daimon — AGNOS agent orchestrator binary.

#[tokio::main]
async fn main() -> daimon::Result<()> {
    #[cfg(feature = "http-forward")]
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    #[cfg(feature = "logging")]
    daimon::logging::try_init();

    let config = daimon::Config::default();

    tracing::info!(
        "daimon v{} starting on port {}",
        env!("CARGO_PKG_VERSION"),
        config.port
    );

    daimon::api::serve(&config).await?;

    Ok(())
}
