use zion_core::network::p2p::{start_p2p_node, CustomEvent};
use blockrock_core::blockchain::Blockchain;
use libp2p::{futures::StreamExt, swarm::SwarmEvent, mdns::Event as MdnsEvent};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let blockchain1 = Arc::new(Mutex::new(Blockchain::new("Node1".to_string())));
    let blockchain2 = Arc::new(Mutex::new(Blockchain::new("Node2".to_string())));

    let mut node1 = start_p2p_node(Arc::clone(&blockchain1)).await?;
    let mut node2 = start_p2p_node(Arc::clone(&blockchain2)).await?;

    node1.listen_on("/ip4/127.0.0.1/tcp/9001".parse()?)?;
    node2.listen_on("/ip4/127.0.0.1/tcp/9002".parse()?)?;

    let node1_handle = tokio::spawn(async move {
        while let Some(event) = node1.next().await {
            if let SwarmEvent::Behaviour(CustomEvent::Mdns(MdnsEvent::Discovered(_))) = event {
                println!("Node1 discovered a peer");
                break;
            }
        }
    });

    let node2_handle = tokio::spawn(async move {
        while let Some(event) = node2.next().await {
            if let SwarmEvent::Behaviour(CustomEvent::Mdns(MdnsEvent::Discovered(_))) = event {
                println!("Node2 discovered a peer");
                break;
            }
        }
    });

    sleep(Duration::from_secs(5)).await;
    node1_handle.abort();
    node2_handle.abort();
    Ok(())
}
