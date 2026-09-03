//! Two nodes on loopback: one seals blocks, the other catches up and then
//! follows along.
//!
//! ```text
//! cargo run --example sync_demo
//! ```

use blockrock_core::blockchain::Blockchain;
use blockrock_core::transaction::Transaction;
use ed25519_dalek::SigningKey;
use libp2p::futures::StreamExt;
use libp2p::multiaddr::Protocol;
use libp2p::swarm::SwarmEvent;
use libp2p::{Multiaddr, Swarm};
use rand::rngs::OsRng;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::timeout;
use zion_core::network::p2p::{start_p2p_node, CustomEvent, MyBehaviour};
use zion_core::network::service::handle_sync_event;
use zion_core::network::sync::{announcement_of, SyncRequest};

const PATIENCE: Duration = Duration::from_secs(20);

type SharedChain = Arc<Mutex<Blockchain>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    // Una rete a singola autorità: entrambi i nodi si fidano della stessa
    // chiave, ma solo il primo conosce Alice.
    let authority = SigningKey::generate(&mut OsRng);
    let alice = SigningKey::generate(&mut OsRng);

    let mut author = Blockchain::new("blockrock".to_string());
    author.register_authority("blockrock", authority.verifying_key());
    author.add_public_key("Alice", alice.verifying_key());
    author.add_block(
        vec![Transaction::new(
            "Alice".to_string(),
            "Bob".to_string(),
            30,
            0,
            &alice,
        )],
        "blockrock",
        &authority,
    )?;

    let mut follower = Blockchain::new("blockrock".to_string());
    follower.register_authority("blockrock", authority.verifying_key());
    println!(
        "genesis condiviso: {}",
        author.genesis_hash().unwrap_or_default()
    );
    println!(
        "prima: nodo1 {} blocchi, nodo2 {} blocchi",
        author.blocks.len(),
        follower.blocks.len()
    );

    let author_chain: SharedChain = Arc::new(Mutex::new(author));
    let follower_chain: SharedChain = Arc::new(Mutex::new(follower));

    let mut author_swarm = start_p2p_node().await?;
    let mut follower_swarm = start_p2p_node().await?;
    let author_peer = *author_swarm.local_peer_id();
    let follower_peer = *follower_swarm.local_peer_id();

    let address = loopback_address(&mut author_swarm).await?;
    println!("nodo1 in ascolto su {}", address);
    follower_swarm.add_peer_address(author_peer, address);

    // 1. Il nodo indietro chiede la catena, come fa alla scoperta di un peer.
    follower_swarm
        .behaviour_mut()
        .sync
        .send_request(&author_peer, SyncRequest::GetChain);
    pump_until(
        (&mut author_swarm, &author_chain),
        (&mut follower_swarm, &follower_chain),
        2,
    )
    .await?;
    report("dopo il sync iniziale", &follower_chain).await;

    // 2. Un blocco sigillato dopo viene annunciato ai peer.
    let announcement = {
        let mut author = author_chain.lock().await;
        author.add_block(
            vec![Transaction::new(
                "Alice".to_string(),
                "Charlie".to_string(),
                20,
                1,
                &alice,
            )],
            "blockrock",
            &authority,
        )?;
        let block = author.blocks.last().unwrap().clone();
        announcement_of(&author, block)
    };
    author_swarm
        .behaviour_mut()
        .sync
        .send_request(&follower_peer, SyncRequest::NewBlock(announcement));
    pump_until(
        (&mut author_swarm, &author_chain),
        (&mut follower_swarm, &follower_chain),
        3,
    )
    .await?;
    report("dopo l'annuncio del blocco", &follower_chain).await;

    Ok(())
}

async fn report(stage: &str, chain: &SharedChain) {
    let chain = chain.lock().await;
    println!(
        "{}: nodo2 ha {} blocchi, Alice {}, Bob {}, Charlie {} (catena valida: {})",
        stage,
        chain.blocks.len(),
        chain.balance_of("Alice"),
        chain.balance_of("Bob"),
        chain.balance_of("Charlie"),
        chain.validate_chain()
    );
}

async fn loopback_address(swarm: &mut Swarm<MyBehaviour>) -> Result<Multiaddr, Box<dyn Error>> {
    swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse()?)?;
    let address = timeout(PATIENCE, async {
        loop {
            if let SwarmEvent::NewListenAddr { address, .. } = swarm.select_next_some().await {
                let loopback = address
                    .iter()
                    .any(|part| matches!(part, Protocol::Ip4(ip) if ip.is_loopback()));
                if loopback {
                    return address;
                }
            }
        }
    })
    .await?;
    Ok(address)
}

/// Fa girare i due nodi finché il secondo non raggiunge `target` blocchi.
async fn pump_until(
    author: (&mut Swarm<MyBehaviour>, &SharedChain),
    follower: (&mut Swarm<MyBehaviour>, &SharedChain),
    target: usize,
) -> Result<(), Box<dyn Error>> {
    let (author_swarm, author_chain) = author;
    let (follower_swarm, follower_chain) = follower;

    timeout(PATIENCE, async {
        loop {
            if follower_chain.lock().await.blocks.len() >= target {
                return;
            }
            tokio::select! {
                event = author_swarm.select_next_some() => {
                    if let SwarmEvent::Behaviour(CustomEvent::Sync(event)) = event {
                        handle_sync_event(author_swarm, author_chain, None, event).await;
                    }
                }
                event = follower_swarm.select_next_some() => {
                    if let SwarmEvent::Behaviour(CustomEvent::Sync(event)) = event {
                        handle_sync_event(follower_swarm, follower_chain, None, event).await;
                    }
                }
            }
        }
    })
    .await?;
    Ok(())
}
