use std::net::SocketAddr;

mod testdatabase;

use testdatabase::TestDatabase;

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
    let (addr, test_db) = spawn_app().await;
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
    let (addr, test_db) = spawn_app().await;
    eprintln!("test app is running ...");
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
        .fetch_one(&test_db.connection_pool().await)
        .await
        .expect("database query failed");

    assert_eq!(data.email.expect("no email found"), expected_email);
    assert_eq!(data.name.expect("no name found"), expected_name);
    eprintln!("test register_returns_200_for_valid_form_data finished");

    test_db.close().await;
}

/// table-driven test or parametrised test for checking failures of subscriptions
#[tokio::test]
async fn register_returns_422_when_data_is_missing() {
    let (addr, test_db) = spawn_app().await;
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

    test_db.close().await;
}

async fn spawn_app() -> (SocketAddr, Box<TestDatabase>) {
    let test_db = setup_database().await;

    let server = registration::startup::run("127.0.0.1:0", test_db.connection_pool().await.clone())
        .expect("could not bind server address");

    let addr = server.local_addr();

    let _ = tokio::spawn(async { server.await });

    println!("server listens under {addr}");

    (addr, test_db)
}
