use anyhow::Error;
use netbench::{Client, ClientConfig, Server, ServerConfig};
use std::env;
use tracing::Level;
use tracing_subscriber::filter::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("netbench=trace"));
    tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .with_env_filter(env_filter)
        .init();

    let mut args = env::args();
    if args.len() < 2 {
        return Err(anyhow::format_err!("Usage netbench {{-c|-s}}"));
    }
    args.next();
    if args.next().unwrap() == "-c" {
        let config = ClientConfig {
            addr: "127.0.0.1:5201".parse().unwrap(),
        };
        let mut c = Client::new(config).await.unwrap();
        c.start_new_test().await.unwrap();
    } else {
        let config = ServerConfig {
            addr: "127.0.0.1:5201".parse().unwrap(),
        };
        let (mut server, _) = Server::new(config).await.unwrap();
        server.accept().await.unwrap();
    }

    Ok(())
}
