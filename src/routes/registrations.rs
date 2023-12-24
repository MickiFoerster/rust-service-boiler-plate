use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Form, Json,
};
use serde::{Deserialize, Serialize};

use crate::startup::AppState;

#[derive(Deserialize, Serialize)]
pub struct RegistrationInput {
    pub name: String,
    pub email: String,
}

pub async fn post_registration_handler(
    State(state): State<AppState>,
    Form(registry_input): Form<RegistrationInput>,
) -> Response {
    tracing::info!(
        "registration handler for name={:#?}, email={:#?}",
        registry_input.name,
        registry_input.email
    );

    let email = registry_input.email.to_lowercase();
    let email = email.trim();
    let name = registry_input.name.trim();

    let result = sqlx::query!(
        r#"
            insert into registrations (
                email,
                name
            )
            values (
                $1,
                $2
            )
            on conflict (email) do nothing
            returning id
    "#,
        email,
        name,
    )
    .fetch_optional(&state.db_pool)
    .await
    .expect("insertion to DB failed");

    match result {
        Some(id) => tracing::info!(
            "name={:#?}, email={:#?} is now registered under ID {:?}",
            registry_input.name,
            registry_input.email,
            id,
        ),
        None => tracing::info!("email '{}' is already registered", registry_input.email),
    }

    (StatusCode::OK, Json(serde_json::json!(registry_input))).into_response()
}
