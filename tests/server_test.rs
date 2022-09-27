use netbench::Client;
use netbench::{ClientConfig, Server, ServerConfig};
#[tokio::test]
async fn test_listen() {
    let mut s = Server::new(ServerConfig {});
    s.listen().await.unwrap();
}

#[tokio::test]
async fn test_connect() {
    let c = Client::new(ClientConfig {});
    c.connect().await.unwrap();
}
