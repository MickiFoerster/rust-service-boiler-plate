use std::net::SocketAddr;

use anyhow::Result;
use axum::{
    routing::{get, post, IntoMakeService},
    Router, Server,
};
use hyper::server::conn::AddrIncoming;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
}

pub fn run(
    service_address: &str,
    db_pool: PgPool,
) -> Result<Server<AddrIncoming, IntoMakeService<Router>>> {
    let addr: SocketAddr = service_address.parse()?;

    let app = Router::new()
        .route("/health_check", get(super::routes::healthcheck))
        .route(
            "/registrations",
            post(super::routes::post_registration_handler),
        )
        .with_state(AppState { db_pool });

    println!("start server ...");
    let server = Server::bind(&addr).serve(app.into_make_service());
    // .with_graceful_shutdown(shutdown_signal()) look example
    //

    println!("bound to address: {:#?}", server.local_addr());

    Ok(server)
}
