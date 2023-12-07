use std::io::Write;

use sqlx::{
    postgres::PgPoolOptions,
    {Connection, Executor},
};

pub struct TestDatabase {
    user: String,
    password: String,
    host: String,
    port: u16,
    database_name: String,
    connection_pool: Option<sqlx::PgPool>,
}

impl TestDatabase {
    pub async fn new(user: String, password: String, host: String, port: u16) -> Self {
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

        test_db
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
        let db_name = self.database_name.clone();
        println!("database {} is going out of scope ...", db_name);

        let mut envs = std::collections::HashMap::new();
        envs.insert("PGUSER", "postgres");
        envs.insert("PGPASSWORD", "password");
        envs.insert("PGHOST", "localhost");
        envs.insert("PGPORT", "5432");
        envs.insert("PGDATABASE", "postgres");

        let mut cmd = std::process::Command::new("psql")
            .envs(&envs)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("psql command failed");

        let mut stdin = cmd
            .stdin
            .take()
            .expect("Failed to take stdin from psql command");

        let db_name2 = db_name.clone();
        std::thread::spawn(move || {
            //let input = format!(" DROP DATABASE \"{}\";\n ", db_name2);
            let input = format!(" \\dt \n ");

            stdin
                .write_all(input.as_bytes())
                .expect("Failed to write to psql command");
        });

        cmd.wait().expect("Failed to execute psql command");

        println!("Test database {} was dropped.", db_name);
    }
}
