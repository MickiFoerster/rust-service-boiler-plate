use std::net::SocketAddr;

use sqlx::{
    postgres::PgPoolOptions,
    {Connection, Executor},
};

struct TestDatabase {
    user: String,
    password: String,
    host: String,
    port: u16,
    database_name: String,
}

impl TestDatabase {
    pub async fn new(user: String, password: String, host: String, port: u16) -> Self {
        let test_db = Self {
            user,
            password,
            host,
            port,
            database_name: uuid::Uuid::new_v4().to_string(),
        };

        let mut conn = sqlx::PgConnection::connect(&test_db.service_uri())
            .await
            .expect("Failed to connect to Postgres");

        conn.execute(format!(r#" CREATE DATABASE "{}"; "#, test_db.database_name).as_str())
            .await
            .expect("Failed to create test database");

        let mut conn = sqlx::PgConnection::connect(&test_db.database_uri())
            .await
            .expect("Failed to connect to Postgres");

        sqlx::migrate!("./migrations")
            .run(&mut conn)
            .await
            .expect("Failed to migrate the database");

        println!("created test database {}", test_db.database_uri());

        test_db
    }

    pub async fn connection_pool(&self) -> sqlx::PgPool {
        PgPoolOptions::new()
            .max_connections(2)
            .connect(&self.database_uri())
            .await
            .expect("cannot connect to the database")
    }

    fn service_uri(&self) -> String {
        let user = &self.user;
        let password = &self.password;
        let host = &self.host;
        let port = self.port;
        format!("postgresql://{user}:{password}@{host}:{port}")
    }

    fn database_uri(&self) -> String {
        let database_name = &self.database_name;
        format!("{}/{}", self.service_uri(), database_name)
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        let db_name = self.database_name.clone();
        println!("database {} is going out of scope ...", db_name);

        println!("Test database {} was dropped.", db_name);
    }
}

async fn setup_database() -> sqlx::Pool<sqlx::Postgres> {
    TestDatabase::new(
        String::from("postgres"),
        String::from("password"),
        String::from("localhost"),
        5432,
    )
    .await
    .connection_pool()
    .await
}

#[tokio::test]
async fn health_check_works() {
    let (addr, _db_pool) = spawn_app().await;
    let url = format!("http://{}/health_check", addr);

    let client = reqwest::Client::new();

    let response = client
        .get(url)
        .send()
        .await
        .expect("Failed to send request.");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[tokio::test]
async fn register_returns_200_for_valid_form_data() {
    let (addr, db_pool) = spawn_app().await;
    let client = reqwest::Client::new();

    let url = format!("http://{}/registrations", addr);

    let expected_name = "Max Mustermann";
    let expected_email = "max.mustermann@gmail.com";

    let body = format!("name={}&email={}", expected_name, expected_email);
    let response = client
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(200u16, response.status().as_u16());

    let data = sqlx::query!("SELECT email, name FROM registrations")
        .fetch_one(&db_pool)
        .await
        .expect("database query failed");

    assert_eq!(data.email.expect("no email found"), expected_email);
    assert_eq!(data.name.expect("no name found"), expected_name);
}

/// table-driven test or parametrised test for checking failures of subscriptions
#[tokio::test]
async fn register_returns_422_when_data_is_missing() {
    let (addr, _db_pool) = spawn_app().await;
    let client = reqwest::Client::new();
    let test_cases = vec![
        ("name=Max%Mustermann", "missing the email"),
        ("email=max.mustermann@gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        let url = format!("http://{}/registrations", addr);
        eprintln!("make call to {url}");
        let response = client
            .post(url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(
            422u16,
            response.status().as_u16(),
            "The API did not fail with 422 Unprocessable Content when the payload was {}",
            error_message
        );
    }
}

async fn spawn_app() -> (SocketAddr, sqlx::Pool<sqlx::Postgres>) {
    let db_pool = setup_database().await;

    let server = registration::startup::run("127.0.0.1:0", db_pool.clone())
        .expect("could not bind server address");

    let addr = server.local_addr();

    let _ = tokio::spawn(async { server.await });

    println!("server listens under {addr}");

    (addr, db_pool)
}
