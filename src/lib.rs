pub mod cli;
pub mod routes;
pub mod startup;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: sqlx::PgPool,
}
