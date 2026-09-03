//! Two real swarms over loopback TCP: a node that is behind catches up, then
//! follows the announcements of the node that seals blocks.

use blockrock_core::blockchain::Blockchain;
use blockrock_core::transaction::Transaction;
use ed25519_dalek::SigningKey;
use libp2p::futures::StreamExt;
use libp2p::multiaddr::Protocol;
use libp2p::swarm::SwarmEvent;
use libp2p::{Multiaddr, Swarm};
use rand::rngs::OsRng;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::timeout;
use zion_core::network::p2p::{start_p2p_node, CustomEvent, MyBehaviour};
use zion_core::network::service::handle_sync_event;
use zion_core::network::sync::{announcement_of, SyncRequest};

const PATIENCE: Duration = Duration::from_secs(20);

type SharedChain = Arc<Mutex<Blockchain>>;

async fn loopback_address(swarm: &mut Swarm<MyBehaviour>) -> Multiaddr {
    swarm
        .listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap())
        .unwrap();

    timeout(PATIENCE, async {
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
    .await
    .expect("the swarm should report a loopback listen address")
}

/// Runs both nodes until the follower reaches `target` blocks, or gives up.
async fn pump_until(
    author: (&mut Swarm<MyBehaviour>, &SharedChain),
    follower: (&mut Swarm<MyBehaviour>, &SharedChain),
    target: usize,
) -> bool {
    let (author_swarm, author_chain) = author;
    let (follower_swarm, follower_chain) = follower;

    timeout(PATIENCE, async {
        loop {
            if follower_chain.lock().await.blocks.len() >= target {
                return true;
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
    .await
    .unwrap_or(false)
}

#[tokio::test]
async fn a_node_catches_up_and_then_follows_new_blocks() {
    let authority = SigningKey::generate(&mut OsRng);
    let alice = SigningKey::generate(&mut OsRng);

    // Both nodes trust the same authority; only the author knows Alice.
    let mut author_chain = Blockchain::new("blockrock".to_string());
    author_chain.register_authority("blockrock", authority.verifying_key());
    author_chain.add_public_key("Alice", alice.verifying_key());

    let mut follower_chain = Blockchain::new("blockrock".to_string());
    follower_chain.register_authority("blockrock", authority.verifying_key());
    assert_eq!(author_chain.genesis_hash(), follower_chain.genesis_hash());

    author_chain
        .add_block(
            vec![Transaction::new(
                "Alice".to_string(),
                "Bob".to_string(),
                30,
                0,
                &alice,
            )],
            "blockrock",
            &authority,
        )
        .unwrap();

    let author_chain: SharedChain = Arc::new(Mutex::new(author_chain));
    let follower_chain: SharedChain = Arc::new(Mutex::new(follower_chain));

    let mut author_swarm = start_p2p_node().await.unwrap();
    let mut follower_swarm = start_p2p_node().await.unwrap();
    let author_peer = *author_swarm.local_peer_id();
    let follower_peer = *follower_swarm.local_peer_id();

    let address = loopback_address(&mut author_swarm).await;
    follower_swarm.add_peer_address(author_peer, address);

    // The follower asks for the chain, exactly as it does on discovery.
    follower_swarm
        .behaviour_mut()
        .sync
        .send_request(&author_peer, SyncRequest::GetChain);

    assert!(
        pump_until(
            (&mut author_swarm, &author_chain),
            (&mut follower_swarm, &follower_chain),
            2,
        )
        .await,
        "the follower should have adopted the author's chain"
    );

    {
        let follower = follower_chain.lock().await;
        assert_eq!(follower.blocks.len(), 2);
        assert_eq!(follower.balance_of("Alice"), 70);
        assert_eq!(follower.balance_of("Bob"), 80);
        // The sender key travelled with the chain, so the block verifies.
        assert_eq!(follower.public_key("Alice"), Some(&alice.verifying_key()));
        assert!(follower.validate_chain());
    }

    // A block sealed afterwards is pushed to the peer, not polled for.
    let announcement = {
        let mut author = author_chain.lock().await;
        author
            .add_block(
                vec![Transaction::new(
                    "Alice".to_string(),
                    "Charlie".to_string(),
                    20,
                    1,
                    &alice,
                )],
                "blockrock",
                &authority,
            )
            .unwrap();
        let block = author.blocks.last().unwrap().clone();
        announcement_of(&author, block)
    };
    author_swarm
        .behaviour_mut()
        .sync
        .send_request(&follower_peer, SyncRequest::NewBlock(announcement));

    assert!(
        pump_until(
            (&mut author_swarm, &author_chain),
            (&mut follower_swarm, &follower_chain),
            3,
        )
        .await,
        "the follower should have appended the announced block"
    );

    let follower = follower_chain.lock().await;
    assert_eq!(follower.blocks.len(), 3);
    assert_eq!(follower.balance_of("Alice"), 50);
    assert_eq!(follower.balance_of("Charlie"), 50);
    assert_eq!(follower.next_nonce("Alice"), 2);
    assert!(follower.validate_chain());
}
