use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(
        short,
        long,
        env,
        default_value_t = String::from("postgresql://postgres:password@localhost:5432/postgres"),
    )]
    pub database_uri: String,

    #[arg(short, long, default_value_t = 8080)]
    pub port: u16,
}
