use std::fmt;

use anyhow::Error;
use clap::{Parser, Subcommand, ValueEnum};
use netbench::{
    Client, ClientConfig, CommonConfig, Protocol, Server, ServerConfig, SizeFormat, TCPTestInfo,
};
use tracing::Level;
use tracing_subscriber::filter::EnvFilter;

const DEFAULT_PORT: u16 = 5202;

#[derive(Debug, Parser)]
#[command(name = "Netbench")]
#[command(author = "Marius M.")]
#[command(about = "network performance benchmark supporting various protocols")]
#[command(version = "0.0.1")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Output in json
    #[arg(long, short)]
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
        /// The server IP to connect to
        host: String,
        #[command(subcommand)]
        proto: ProtocolCommands,
        /// time in seconds to transmit
        #[arg(long, short, default_value_t = 10, conflicts_with = "bytes")]
        time: u32,
        /// number of bytes to transmit, 0 for unlimited
        #[arg(long, short = 'n')]
        bytes: Option<u64>,
        /// port to listen on
        #[arg(long, short, default_value_t = DEFAULT_PORT, value_parser = clap::value_parser!(u16).range(1..))]
        port: u16,
        /// Direction to send
        #[arg(long, short, default_value_t = Direction::Uni)]
        direction: Direction,
    },
    Server {
        #[arg(default_value_t = String::from("0.0.0.0"))]
        host: String,
        /// port to listen on
        #[arg(long, short, default_value_t = DEFAULT_PORT, value_parser = clap::value_parser!(u16).range(1..))]
        port: u16,
    },
}

#[derive(Debug, Subcommand)]
#[allow(clippy::upper_case_acronyms)]
enum ProtocolCommands {
    TCP {
        /// Set the length of the send/rcv buffers
        #[arg(long, short, default_value_t = String::from("128k"))]
        length: String,
    },
    UDP,
    DCCP,
    SCTP,
    QUIC,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, PartialOrd)]
enum Direction {
    /// Unidirectional: send from client to server
    Uni,
    /// Unidirectional but reverse: send from server to client
    Rev,
    /// Bidirectional: send from client to server and server to client
    Bi,
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_possible_value().unwrap().get_name().fmt(f)
    }
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
        Commands::Client {
            host, port, proto, ..
        } => {
            let proto = match proto {
                ProtocolCommands::TCP { length } => {
                    let length = netbench::parse_u64_from_suffix(&length).unwrap();
                    Protocol::TCP(TCPTestInfo {
                        recv_buf_size: length,
                        send_buf_size: length,
                    })
                }
                _ => todo!("only TCP is implemented right now"),
            };
            let addr = format!("{}:{}", host, port);
            let config = ClientConfig {
                addr: addr.parse().unwrap(),
                common: common_config,
                bw: None,
                proto,
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

#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Cli::command().debug_assert();
}
