use clap::Parser;
use registration::cli::Args;
use registration::startup::run_server;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "registration=debug,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    let close_tx = run_server(args).await?;

    tracing::debug!("waiting for {} tasks to finish", close_tx.receiver_count());
    close_tx.closed().await;

    Ok(())
}
