use axum::{
    routing::{get, post},
    Router,
};
use clap::Parser;
use registration::{cli::Args, AppState};
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "registry-service-example=debug,tower_http=debug".into()),
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

    // see axum example axum/examples/graceful-shutdown/src/main.rs

    let app = Router::new()
        .route("/health_check", get(registration::routes::healthcheck))
        .route(
            "/registrations",
            post(registration::routes::post_registration_handler),
        )
        .layer((
            tower_http::trace::TraceLayer::new_for_http(),
            tower_http::timeout::TimeoutLayer::new(std::time::Duration::from_secs(10)),
        ))
        .with_state(AppState { db_pool });

    println!("bind server address ...");
    let listener = tokio::net::TcpListener::bind(service_address).await?;

    //let server = Server::bind(&addr).serve(app.into_make_service());

    //println!("bound to address: {:#?}", server.local_addr());
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    eprintln!("Received termination signal shutting down");
}
