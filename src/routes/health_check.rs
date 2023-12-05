use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub async fn healthcheck() -> Response {
    println!("healthcheck requested");
    (StatusCode::OK).into_response()
}
