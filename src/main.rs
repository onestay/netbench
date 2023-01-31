use std::fmt;

use anyhow::Error;
use clap::{Parser, Subcommand, ValueEnum};
use netbench::{Client, ClientConfig, CommonConfig, Server, ServerConfig, SizeFormat};
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
    #[arg(
        long,
        short,
        default_value_t = false,
        default_missing_value = "true",
        num_args = 0
    )]
    json: bool,
    #[arg(
        long,
        value_name = "WHEN",
        default_value_t = ColorWhen::Auto,
        value_enum,
        num_args = 0..=1,
        default_missing_value = "always",
    )]
    color: ColorWhen,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Client {
        host: String,
        /// The port to listen on
        #[arg(long, short, default_value_t = DEFAULT_PORT)]
        port: u16,
        /// Set the length of the send/rcv buffers
        #[arg(long, short, default_value_t = String::from("128k"))]
        length: String,
    },
    Server {
        host: String,
        /// The port to listen on
        #[arg(long, short, default_value_t = DEFAULT_PORT)]
        port: u16,
    },
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, PartialOrd)]
enum ColorWhen {
    Always,
    Auto,
    Never,
}

impl fmt::Display for ColorWhen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_possible_value().unwrap().get_name().fmt(f)
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(""));
    tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .with_env_filter(env_filter)
        .init();

    let matches = Cli::parse();
    let common_config = CommonConfig {
        file: None,
        format: SizeFormat::Auto,
    };

    match matches.command {
        Commands::Client { host, port, .. } => {
            let addr = format!("{}:{}", host, port);
            let config = ClientConfig {
                addr: addr.parse().unwrap(),
                common: common_config,
                bw: None,
            };

            let mut c = Client::new(config).await.unwrap();
            c.start_new_test().await.unwrap();
        }

        Commands::Server { host, port } => {
            let addr = format!("{}:{}", host, port);
            let config = ServerConfig {
                addr: addr.parse().unwrap(),
                common: common_config,
            };
            let (mut server, _) = Server::new(config).await.unwrap();
            server.accept().await.unwrap();
        }
    }

    Ok(())
}
