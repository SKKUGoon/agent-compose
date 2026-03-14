mod config;
mod io;
mod loader;
mod terminal;
mod provider;
mod resolver;
mod runtime;
mod schema;
mod server;

#[tokio::main]
async fn main() {
    if let Err(err) = terminal::cli::run_cli().await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
