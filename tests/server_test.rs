use netbench::{Client, ClientConfig, Server, ServerConfig};
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_start_new_test() {
    let (mut server, ch) = Server::new(ServerConfig { addr: "127.0.0.1:8080".parse().unwrap() }).await.unwrap();
    let handle = tokio::spawn(async move {
        server.accept().await.unwrap();
    });
    let client = Client::new(ClientConfig {addr: "127.0.0.1:8080".parse().unwrap()}).await.unwrap();
    client.start_new_test().await.unwrap();
    sleep(Duration::from_secs(1)).await;
    ch.send(netbench::ControlMessage::Stop).await.unwrap();
    handle.await.unwrap();
}
