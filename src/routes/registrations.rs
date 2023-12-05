use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Form, Json,
};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Deserialize, Serialize)]
pub struct RegistrationInput {
    pub name: String,
    pub email: String,
}

pub async fn post_registration_handler(
    State(state): State<AppState>,
    Form(registry_input): Form<RegistrationInput>,
) -> Response {
    //tracing::info!("registry_input:{:#?}", registry_input);
    eprintln!("registry_input:name:{:#?}", registry_input.name);
    eprintln!("registry_input:email:{:#?}", registry_input.email);

    let result = sqlx::query!(
        r#"
       select * from registrations
                              "#
    )
    .fetch_optional(&state.db_pool)
    .await
    .expect("DB query failed");
    (StatusCode::OK, Json(serde_json::json!(registry_input))).into_response()
}
