use clap::Parser;

#[tokio::main]
async fn main() {
    let cli = oracode::Cli::parse();
    if let Err(error) = oracode::run(cli).await {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
