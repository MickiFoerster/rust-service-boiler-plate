use sqlx::{
    postgres::PgPoolOptions,
    {Connection, Executor},
};

#[derive(Clone)]
pub struct TestDatabase {
    user: String,
    password: String,
    host: String,
    port: u16,
    database_name: String,
    connection_pool: Option<sqlx::PgPool>,
}

impl TestDatabase {
    pub async fn new(user: String, password: String, host: String, port: u16) -> Box<Self> {
        let mut test_db = Self {
            user,
            password,
            host,
            port,
            database_name: uuid::Uuid::new_v4().to_string(),
            connection_pool: None,
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

        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(&test_db.database_uri())
            .await
            .expect("cannot connect to the database");

        test_db.connection_pool = Some(pool);

        Box::new(test_db)
    }

    pub async fn close(&mut self) {
        let pool = self.connection_pool.take();
        if let Some(pool) = pool {
            pool.close().await;
        }

        let mut conn = sqlx::PgConnection::connect(&self.service_uri())
            .await
            .expect("Failed to connect to Postgres");

        conn.execute(format!(r#" DROP DATABASE "{}"; "#, self.database_name).as_str())
            .await
            .expect("Failed to drop test database");

        eprintln!("database {} dropped", self.database_name);
    }

    pub async fn connection_pool(&self) -> sqlx::PgPool {
        self.connection_pool
            .as_ref()
            .expect("there is no connection pool available")
            .clone()
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
        if self.connection_pool.is_some() {
            eprintln!("test database connection pool was not closed");
        } else {
            eprintln!("test database was already closed");
        }
    }
}
