use clap::Parser;

use registration::{cli::Args, startup::run};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let addr = format!("127.0.0.1:{}", args.port);
    println!("database uri: {}", args.database_uri);
    println!("configured address: {}", addr);

    let db_pool = PgPoolOptions::new()
        .max_connections(32)
        .connect(&args.database_uri)
        .await
        .expect("cannot connect to the database");

    Ok(run(&addr, db_pool)?.await?)
}
