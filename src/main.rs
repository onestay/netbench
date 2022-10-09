use anyhow::Error;
use netbench::{Direction, Protocol, Role, Test, TestSetup};
use std::env;
use time::Duration;
#[tokio::main]
async fn main() -> Result<(), Error> {
    let mut args = env::args();
    if args.len() < 2 {
        return Err(anyhow::format_err!("Usage netbench {{-c|-s}}"));
    }
    args.next();
    if args.next().unwrap() == "-c" {
        let setup_client = TestSetup {
            role: Role::Client,
            direction: Direction::ClientToServer,
            protocol: Protocol::TCP,
            duration: Duration::new(10, 0),
            intervals: Duration::new(1, 0),
            addr: "10.0.0.10:5201".parse()?,
        };

        let test_client = Test::new(setup_client).await?;

        test_client.run().await?;
    } else {
        let setup_server = TestSetup {
            role: Role::Server,
            direction: Direction::ClientToServer,
            protocol: Protocol::TCP,
            duration: Duration::new(10, 0),
            intervals: Duration::new(1, 0),
            addr: "10.0.0.10:5201".parse()?,
        };

        let test_server = Test::new(setup_server).await?;

        test_server.run().await?;
    }

    Ok(())
}
