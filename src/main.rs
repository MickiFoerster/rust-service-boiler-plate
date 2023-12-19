use axum::{
    extract::Request,
    routing::{get, post},
    Router,
};
use clap::Parser;
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;
use registration::{cli::Args, AppState};
use sqlx::postgres::PgPoolOptions;
use tower::Service;
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

        let tower_service = app.clone();
        let close_rx = close_rx.clone();

        tokio::spawn(async move {
            // Hyper has its own `AsyncRead` and `AsyncWrite` traits and doesn't use tokio.
            // `TokioIo` converts between them.
            let socket = TokioIo::new(socket);

            // Hyper also has its own `Service` trait and doesn't use tower. We can use
            // `hyper::service::service_fn` to create a hyper `Service` that calls our app through
            // `tower::Service::call`.
            let hyper_service = hyper::service::service_fn(move |request: Request<Incoming>| {
                // We have to clone `tower_service` because hyper's `Service` uses `&self` whereas
                // tower's `Service` requires `&mut self`.
                //
                // We don't need to call `poll_ready` since `Router` is always ready.
                tower_service.clone().call(request)
            });

            // `hyper_util::server::conn::auto::Builder` supports both http1 and http2 but doesn't
            // support graceful so we have to use hyper directly and unfortunately pick between
            // http1 and http2.
            let conn = hyper::server::conn::http1::Builder::new()
                .serve_connection(socket, hyper_service)
                // `with_upgrades` is required for websockets.
                .with_upgrades();

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
        });
    }
    eprintln!("exit from loop");

    drop(close_rx);

    drop(listener);

    tracing::debug!("waiting for {} tasks to finish", close_tx.receiver_count());
    close_tx.closed().await;

    eprintln!("finish main");
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
}
