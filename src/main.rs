use anyhow::Error;
use clap::{Parser, Subcommand};
use netbench::{Client, ClientConfig, Server, ServerConfig};
use tracing::Level;
use tracing_subscriber::filter::EnvFilter;

const DEFAULT_PORT: u16 = 5202;

#[derive(Debug, Parser)]
#[command(name = "netbench")]
#[command(about = "network performance benchmark supporting various protocols")]
#[command(version = "0.0.1")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Output in json
    #[arg(long, short, default_value_t = false)]
    json: bool,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Client {
        host: String,
        /// The port to listen on
        #[arg(long, short, default_value_t = DEFAULT_PORT)]
        port: u16,
    },
    Server {
        host: String,
        /// The port to listen on
        #[arg(long, short, default_value_t = DEFAULT_PORT)]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(""));
    tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .with_env_filter(env_filter)
        .init();

    let matches = Cli::parse();
    match matches.command {
        Commands::Client { host, port } => {
            let addr = format!("{}:{}", host, port);
            let config = ClientConfig {
                addr: addr.parse().unwrap(),
            };
            let mut c = Client::new(config).await.unwrap();
            c.start_new_test().await.unwrap();
        }

        Commands::Server { host, port } => {
            let addr = format!("{}:{}", host, port);
            let config = ServerConfig {
                addr: addr.parse().unwrap(),
            };
            let (mut server, _) = Server::new(config).await.unwrap();
            server.accept().await.unwrap();
        }
    }

    Ok(())
}
