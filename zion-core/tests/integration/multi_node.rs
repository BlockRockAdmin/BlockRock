use zion_core::network::p2p::start_p2p;
use zion_core::blockchain::Blockchain;
use tokio::time::{sleep, Duration};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_multi_node_discovery() {
    tracing_subscriber::fmt::init();

    let blockchain1 = Arc::new(Mutex::new(Blockchain::new("Node1".to_string())));
    let blockchain2 = Arc::new(Mutex::new(Blockchain::new("Node2".to_string())));

    let mut node1 = start_p2p(blockchain1).await.unwrap();
    let mut node2 = start_p2p(blockchain2).await.unwrap();

    node1.listen_on("/ip4/127.0.0.1/tcp/9001".parse().unwrap()).unwrap();
    node2.listen_on("/ip4/127.0.0.1/tcp/9002".parse().unwrap()).unwrap();

    let node1_handle = tokio::spawn(async move {
        while let Some(event) = node1.next().await {
            if let SwarmEvent::Behaviour(ZionBehaviourEvent::Mdns { peers, .. }) = event {
                assert!(!peers.is_empty(), "Node1 dovrebbe scoprire peer");
                break;
            }
        }
    });

    let node2_handle = tokio::spawn(async move {
        while let Some(event) = node2.next().await {
            if let SwarmEvent::Behaviour(ZionBehaviourEvent::Mdns { peers, .. }) = event {
                assert!(!peers.is_empty(), "Node2 dovrebbe scoprire peer");
                break;
            }
        }
    });

    sleep(Duration::from_secs(5)).await;

    node1_handle.abort();
    node2_handle.abort();
}
