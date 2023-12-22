use std::net::SocketAddr;

use axum::{
    extract::Request,
    routing::{get, post},
    Router,
};
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpStream;
use tower::Service;

use crate::{
    cli::Args,
    routes::{healthcheck, post_registration_handler},
};

#[derive(Clone)]
pub struct AppState {
    pub db_pool: sqlx::PgPool,
}

pub async fn run_server(args: Args) -> anyhow::Result<tokio::sync::watch::Sender<()>> {
    let service_address = format!("127.0.0.1:{}", args.port);
    println!("database uri: {}", args.database_uri);
    println!("configured address: {}", service_address);

    let db_pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&args.database_uri)
        .await
        .expect("cannot connect to the database");

    let router = Router::new()
        .route("/health_check", get(healthcheck))
        .route("/registrations", post(post_registration_handler))
        .layer((
            tower_http::trace::TraceLayer::new_for_http(),
            tower_http::timeout::TimeoutLayer::new(std::time::Duration::from_secs(10)),
        ))
        .with_state(AppState { db_pool });

    println!("bind server address ...");
    let listener = tokio::net::TcpListener::bind(service_address).await?;

    let (close_tx, close_rx) = tokio::sync::watch::channel(());

    loop {
        let (socket, remote_addr) = tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok(stream) => stream,
                    Err(e) => {
                        tracing::error!("failed to accept connection: {e}");
                        continue;
                    }
                }
            }
            _ = shutdown_signal() => {
                tracing::debug!("signal received, not accepting new connections");
                break;
            }
        };

        tracing::debug!("connection from {remote_addr} accepted");

        let tower_service = router.clone();
        let close_rx = close_rx.clone();

        tokio::spawn(async move { handle_client(socket, remote_addr, tower_service, close_rx) });
    }
    eprintln!("exit from loop");

    drop(close_rx);

    drop(listener);

    Ok(close_tx)
}

async fn handle_client(
    socket: TcpStream,
    remote_addr: SocketAddr,
    tower_service: Router,
    close_rx: tokio::sync::watch::Receiver<()>,
) {
    let socket = TokioIo::new(socket);

    let hyper_service = hyper::service::service_fn(move |request: Request<Incoming>| {
        tower_service.clone().call(request)
    });

    let conn = hyper::server::conn::http1::Builder::new().serve_connection(socket, hyper_service);
    // `graceful_shutdown` requires a pinned connection.
    let mut conn = std::pin::pin!(conn);

    loop {
        tokio::select! {
            // Poll the connection. This completes when the client has closed the
            // connection, graceful shutdown has completed, or we encounter a TCP error.
            result = conn.as_mut() => {
                if let Err(err) = result {
                    tracing::debug!("failed to serve connection: {err:#}");
                }
                break;
            }
            // Start graceful shutdown when we receive a shutdown signal.
            //
            // We use a loop to continue polling the connection to allow requests to finish
            // after starting graceful shutdown. Our `Router` has `TimeoutLayer` so
            // requests will finish after at most 10 seconds.
            _ = shutdown_signal() => {
                eprintln!("signal received, starting graceful shutdown");
                conn.as_mut().graceful_shutdown();
            }
        }
    }

    tracing::debug!("connection {remote_addr} closed");

    // Drop the watch receiver to signal to `main` that this task is done.
    drop(close_rx);
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
}
