//! Daimon — AGNOS agent orchestrator binary.

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
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
