use std::net::SocketAddr;

use axum::{
    body::Body,
    extract::Request,
    routing::{get, post},
    Router,
};
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;
use tokio::{net::TcpStream, task::JoinHandle};
use tower::Service;

use crate::routes::{healthcheck, post_registration_handler};

#[derive(Clone)]
pub struct AppState {
    pub db_pool: sqlx::PgPool,
}

pub async fn run_server(
    service_address: &str,
    db_pool: sqlx::PgPool,
) -> anyhow::Result<(JoinHandle<()>, SocketAddr, tokio::sync::watch::Sender<()>)> {
    let router = Router::new()
        .route("/health_check", get(healthcheck))
        .route("/registrations", post(post_registration_handler))
        .layer(
            tower_http::trace::TraceLayer::new_for_http().make_span_with(
                |request: &Request<Body>| {
                    let request_id = uuid::Uuid::new_v4();
                    let version = format!("{:#?}", request.version());

                    tracing::span!(
                        tracing::Level::INFO,
                        "request",
                        method = tracing::field::display(request.method()),
                        uri = tracing::field::display(request.uri()),
                        version = tracing::field::display(version),
                        request_id = tracing::field::display(request_id),
                    )
                },
            ),
        )
        .layer((
            tower_http::trace::TraceLayer::new_for_http(),
            tower_http::timeout::TimeoutLayer::new(std::time::Duration::from_secs(10)),
        ))
        .with_state(AppState { db_pool });

    tracing::debug!("bind server address ...");
    let listener = tokio::net::TcpListener::bind(service_address).await?;
    // Now socket is open and client can already connect to it. Therefore, no signal that server
    // starts to accept connection is needed.
    let addr = listener.local_addr()?;

    let (close_tx, close_rx) = tokio::sync::watch::channel(());

    let join_handle = tokio::spawn(async move {
        loop {
            tracing::info!("server now waits foc incoming connections ...");

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

            tracing::info!("connection from {remote_addr} accepted");

            let tower_service = router.clone();
            let close_rx = close_rx.clone();

            tokio::spawn(async move {
                handle_client(socket, remote_addr, tower_service, close_rx).await;
            });
        }
        tracing::debug!("exit from loop which accepts connections");
    });

    Ok((join_handle, addr, close_tx))
}

async fn handle_client(
    socket: TcpStream,
    remote_addr: SocketAddr,
    tower_service: Router,
    close_rx: tokio::sync::watch::Receiver<()>,
) {
    tracing::info!("handle client connection from {remote_addr}");
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
                    tracing::error!("failed to serve connection: {err:#}");
                }
                break;
            }
            // Start graceful shutdown when we receive a shutdown signal.
            //
            // We use a loop to continue polling the connection to allow requests to finish
            // after starting graceful shutdown. Our `Router` has `TimeoutLayer` so
            // requests will finish after at most 10 seconds.
            _ = shutdown_signal() => {
                tracing::error!("signal received, starting graceful shutdown");
                conn.as_mut().graceful_shutdown();
            }
        }
    }

    tracing::info!("client connection {remote_addr} closed");

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
