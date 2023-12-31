use std::net::SocketAddr;

mod testdatabase;

use registration::startup::run_server;
use testdatabase::TestDatabase;
use tokio::{sync::watch::Sender, task::JoinHandle};

async fn setup_database() -> Box<TestDatabase> {
    TestDatabase::new(
        String::from("postgres"),
        String::from("password"),
        String::from("localhost"),
        5432,
    )
    .await
}

#[tokio::test]
async fn health_check_works() {
    let (_, addr, _, mut test_db) = spawn_app().await;
    let url = format!("http://{}/health_check", addr);

    let client = reqwest::Client::new();

    let response = client
        .get(url)
        .send()
        .await
        .expect("Failed to send request.");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());

    test_db.close().await;
}

#[tokio::test]
async fn register_returns_200_for_valid_form_data() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "registration=debug,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let (_, addr, _, mut test_db) = spawn_app().await;
    tracing::debug!("test app is running ...");
    let client = reqwest::Client::new();

    let url = format!("http://{}/registrations", addr);

    let expected_name = "Max Mustermann";
    let expected_email = "max.mustermann@gmail.com";

    let body = format!("name={}&email={}", expected_name, expected_email);
    tracing::debug!("connection opened on endpoint {url}");
    let response = client
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(200u16, response.status().as_u16());

    let data = sqlx::query!("SELECT email, name FROM registrations")
        .fetch_one(&test_db.connection_pool().await)
        .await
        .expect("database query failed");

    assert_eq!(data.email.expect("no email found"), expected_email);
    assert_eq!(data.name.expect("no name found"), expected_name);

    test_db.close().await;
}

/// table-driven test or parametrised test for checking failures of subscriptions
#[tokio::test]
async fn register_returns_422_when_data_is_missing() {
    let (_, addr, _, mut test_db) = spawn_app().await;
    let client = reqwest::Client::new();
    let test_cases = vec![
        ("name=Max%Mustermann", "missing the email"),
        ("email=max.mustermann@gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        let url = format!("http://{}/registrations", addr);
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

    test_db.close().await;
}

async fn spawn_app() -> (JoinHandle<()>, SocketAddr, Sender<()>, Box<TestDatabase>) {
    let test_db = setup_database().await;

    let (join_handle, local_addr, close_tx) =
        run_server("127.0.0.1:0", test_db.connection_pool().await)
            .await
            .expect("could not bind server address");

    (join_handle, local_addr, close_tx, test_db)
}
