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

#[tracing::instrument(name = "Register a new user with email", skip(form, state), 

    fields(
        // FIXME: request id should later be moved into layer from tower service
        subscriber_name = %form.name,
        subscriber_email = %form.email,
        ),
)]
pub async fn post_registration_handler(
    State(state): State<AppState>,
    Form(form): Form<RegistrationInput>,
) -> Response {
    let email = form.email.to_lowercase();
    let email = email.trim();
    let name = form.name.trim();

    let result = match insert_user(&state.db_pool, name, email).await {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not save user data",
            )
                .into_response()
        }
    };

    match result {
        Some(id) => tracing::info!(
            "name={:#?}, email={:#?} is now registered under ID {:?}",
            form.name,
            form.email,
            id,
        ),
        None => tracing::info!(": email '{}' is already registered", form.email),
    }

    (StatusCode::OK, Json(serde_json::json!(form))).into_response()
}

#[tracing::instrument(name = "Store data in database", skip(pool, name, email))]
async fn insert_user(
    pool: &sqlx::PgPool,
    name: &str,
    email: &str,
) -> Result<Option<uuid::Uuid>, sqlx::Error> {
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
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute SQL query: {e}");

        e
    })?
    .map(|r| r.id);

    Ok(result)
}
