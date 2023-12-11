use std::time::Duration;

use clap::Parser;

use registration::{cli::Args, startup::run};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let addr = format!("127.0.0.1:{}", args.port);
    println!("database uri: {}", args.database_uri);
    println!("configured address: {}", addr);

    let db_pool = PgPoolOptions::new()
        .max_connections(32)
        .connect(&args.database_uri)
        .await
        .expect("cannot connect to the database");

    let handle = axum_server::Handle::new();
    let shutdown_future = shutdown_signal(handle.clone());

    Ok(run(&addr, db_pool)?
        .with_graceful_shutdown(shutdown_future)
        .await?)
}

async fn shutdown_signal(handle: axum_server::Handle) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    eprintln!("Received termination signal shutting down");
    handle.graceful_shutdown(Some(Duration::from_secs(10))); // 10 secs is how long docker will wait
}
