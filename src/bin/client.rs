use anyhow::Result;
use netbench::TestBuilder;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let client = TestBuilder::client("127.0.0.1:3213")?.build().await?;

    client.run().await?;

    Ok(())
}
