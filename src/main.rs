use clap::Parser;
use registration::cli::Args;
use registration::startup::run_server;
use sqlx::postgres::PgPoolOptions;
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

    let service_address = format!("127.0.0.1:{}", args.port);
    println!("database uri: {}", args.database_uri);
    println!("configured address: {}", service_address);

    let db_pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&args.database_uri)
        .await
        .expect("cannot connect to the database");

    let (local_addr, close_tx) = run_server(&service_address, db_pool).await?;
    tracing::info!("Server listening on {local_addr}");

    tracing::info!("waiting for {} tasks to finish", close_tx.receiver_count());
    close_tx.closed().await;

    Ok(())
}
