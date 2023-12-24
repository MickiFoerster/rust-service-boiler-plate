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
    //tracing::info!("registry_input:{:#?}", registry_input);
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
    "#,
        email,
        name,
    )
    .execute(&state.db_pool)
    .await
    .expect("insertion to DB failed");

    std::thread::sleep(std::time::Duration::from_secs(5));
    tracing::debug!("{:#?}", result);

    (StatusCode::OK, Json(serde_json::json!(registry_input))).into_response()
}
