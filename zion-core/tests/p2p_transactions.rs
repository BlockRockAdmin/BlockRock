//! Actual loopback swarms, the real REST admission path and the node's gossip
//! driver: leaf -> relay -> validator, then the confirmed block comes back.
use blockrock_core::blockchain::Blockchain;
use blockrock_core::transaction::Transaction;
use ed25519_dalek::SigningKey;
use libp2p::futures::StreamExt;
use libp2p::multiaddr::Protocol;
use libp2p::swarm::SwarmEvent;
use libp2p::{Multiaddr, PeerId};
use rand::rngs::OsRng;
use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::time::{sleep, timeout};
use zion_core::api::rest::TronService;
use zion_core::identity::NodeIdentity;
use zion_core::ledger;
use zion_core::monitoring::shared_metabolic_status;
use zion_core::network::p2p::{start_p2p_node, CustomEvent};
use zion_core::network::service::handle_sync_event;
use zion_core::network::sync::{BlockAnnouncement, SyncRequest};
use zion_core::network::transactions::TransactionGossip;
use zion_core::server::{self, ServerContext};
use zion_core::storage::ChainStore;

struct Node {
    chain: Arc<Mutex<Blockchain>>,
    identity: NodeIdentity,
    store: ChainStore,
    peer: PeerId,
    address: Multiaddr,
    connect: mpsc::Sender<(PeerId, Multiaddr)>,
    blocks: mpsc::Sender<BlockAnnouncement>,
    task: tokio::task::JoinHandle<()>,
    _dir: TempDir,
}

impl Drop for Node {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Node {
    async fn start(chain: Blockchain, name: &str, key: SigningKey) -> Self {
        let dir = TempDir::new().unwrap();
        let store = ChainStore::new(dir.path().join("chain.json"));
        store.save(&chain).unwrap();
        let chain = Arc::new(Mutex::new(chain));
        let identity = NodeIdentity {
            name: name.into(),
            signing_key: key,
        };
        let mut swarm = start_p2p_node().await.unwrap();
        swarm
            .listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap())
            .unwrap();
        let peer = *swarm.local_peer_id();
        let address = timeout(Duration::from_secs(20), async {
            loop {
                if let SwarmEvent::NewListenAddr { address, .. } = swarm.select_next_some().await {
                    if address
                        .iter()
                        .any(|p| matches!(p, Protocol::Ip4(ip) if ip.is_loopback()))
                    {
                        break address;
                    }
                }
            }
        })
        .await
        .unwrap();
        let (connect, mut connections) = mpsc::channel::<(PeerId, Multiaddr)>(8);
        let (blocks, mut announcements) = mpsc::channel::<BlockAnnouncement>(64);
        let task_chain = Arc::clone(&chain);
        let task_store = store.clone();
        let task = tokio::spawn(async move {
            let mut connected = HashSet::new();
            let mut gossip = TransactionGossip::default();
            let mut interval = tokio::time::interval(Duration::from_millis(50));
            loop {
                tokio::select! {
                    _ = interval.tick() => {}
                    Some((peer, address)) = connections.recv() => {
                        swarm.add_peer_address(peer, address);
                        swarm.behaviour_mut().sync.send_request(&peer, SyncRequest::GetChain);
                    }
                    Some(block) = announcements.recv() => {
                        for peer in &connected {
                            swarm.behaviour_mut().sync.send_request(peer, SyncRequest::NewBlock(block.clone()));
                        }
                    }
                    event = swarm.select_next_some() => match event {
                        SwarmEvent::Behaviour(CustomEvent::Sync(event)) => {
                            handle_sync_event(&mut swarm, &task_chain, Some(&task_store), event).await;
                        }
                        SwarmEvent::Behaviour(CustomEvent::Transaction(event)) => {
                            gossip.handle_event(&mut swarm, &task_chain, event).await;
                        }
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            if connected.insert(peer_id) {
                                swarm.behaviour_mut().sync.send_request(&peer_id, SyncRequest::GetChain);
                            }
                        }
                        SwarmEvent::ConnectionClosed { peer_id, num_established: 0, .. } => { connected.remove(&peer_id); }
                        _ => {}
                    }
                }
                gossip.drive(&mut swarm, &*task_chain.lock().await, &connected);
            }
        });
        Node {
            chain,
            identity,
            store,
            peer,
            address,
            connect,
            blocks,
            task,
            _dir: dir,
        }
    }

    async fn connect_to(&self, other: &Node) {
        self.connect
            .send((other.peer, other.address.clone()))
            .await
            .unwrap();
    }

    async fn rest(&self) -> Client {
        let (sensor_tx, _rx) = broadcast::channel(8);
        Client::tracked(server::build(ServerContext {
            blockchain: Arc::clone(&self.chain),
            identity: self.identity.clone(),
            store: self.store.clone(),
            sensor_tx,
            metabolic_status: shared_metabolic_status(),
            tron_service: TronService::new("test-only".into(), "test-only".into()),
            block_announcer: self.blocks.clone(),
        }))
        .await
        .unwrap()
    }

    async fn wait_pending(&self, count: usize) {
        timeout(Duration::from_secs(20), async {
            loop {
                if self.chain.lock().await.pending_count() == count {
                    break;
                }
                sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("pending transactions should propagate");
    }

    async fn wait_blocks(&self, count: usize) {
        timeout(Duration::from_secs(20), async {
            loop {
                if self.chain.lock().await.blocks.len() == count {
                    break;
                }
                sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("confirmed blocks should propagate back");
    }
}

#[tokio::test]
async fn a_validator_behind_the_dialling_follower_catches_up_before_accepting_its_next_nonce() {
    let authority = SigningKey::generate(&mut OsRng);
    let alice = SigningKey::generate(&mut OsRng);
    let mut base = Blockchain::new_single("validator".into());
    base.register_authority("validator", authority.verifying_key());
    let mut newer = base.clone();
    newer.add_public_key("Alice", alice.verifying_key());
    newer
        .add_block(
            vec![Transaction::new(
                "Alice".into(),
                "Bob".into(),
                10,
                0,
                &alice,
            )],
            "validator",
            &authority,
        )
        .unwrap();
    let author = Node::start(base, "validator", authority).await;
    let leaf = Node::start(newer, "leaf", SigningKey::generate(&mut OsRng)).await;
    let client = leaf.rest().await;
    let tx = Transaction::new("Alice".into(), "Bob".into(), 10, 1, &alice);
    let response = client
        .post("/transactions")
        .header(ContentType::JSON)
        .body(
            json!({
                "sender": "Alice", "receiver": "Bob", "amount": 10, "nonce": 1,
                "signature": hex::encode(tx.signature.unwrap().to_bytes())
            })
            .to_string(),
        )
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    // Only the newer follower initiates this connection.
    leaf.connect_to(&author).await;
    author.wait_pending(1).await;
    assert_eq!(author.chain.lock().await.blocks.len(), 2);
    assert_eq!(
        ledger::seal_pending(
            &author.chain,
            &author.identity,
            &author.store,
            &author.blocks
        )
        .await
        .unwrap(),
        Some(2)
    );
    leaf.wait_blocks(3).await;
    let chain = leaf.chain.lock().await;
    assert_eq!(chain.pending_count(), 0);
    assert_eq!(chain.balance_of("Alice"), 80);
    assert!(chain.validate_chain());
}

#[tokio::test]
async fn rest_on_a_non_validator_reaches_a_validator_through_a_relay_and_is_confirmed_once() {
    let authority = SigningKey::generate(&mut OsRng);
    let alice = SigningKey::generate(&mut OsRng);
    let mut base = Blockchain::new_single("validator".into());
    base.register_authority("validator", authority.verifying_key());
    let author = Node::start(base.clone(), "validator", authority).await;
    let relay = Node::start(base.clone(), "relay", SigningKey::generate(&mut OsRng)).await;
    let leaf = Node::start(base, "leaf", SigningKey::generate(&mut OsRng)).await;
    leaf.chain
        .lock()
        .await
        .add_public_key("Alice", alice.verifying_key());
    let client = leaf.rest().await;
    // Queued before any peer is connected: connection backfill must work too.
    for nonce in 0..3 {
        let tx = Transaction::new("Alice".into(), "Bob".into(), 10, nonce, &alice);
        let response = client
            .post("/transactions")
            .header(ContentType::JSON)
            .body(
                json!({
                    "sender": "Alice", "receiver": "Bob", "amount": 10, "nonce": nonce,
                    "signature": hex::encode(tx.signature.unwrap().to_bytes())
                })
                .to_string(),
            )
            .dispatch()
            .await;
        assert_eq!(response.status(), Status::Ok);
    }
    leaf.connect_to(&relay).await;
    relay.connect_to(&author).await;
    author.wait_pending(3).await;
    assert_eq!(author.chain.lock().await.balance_of("Alice"), 100);
    assert_eq!(
        ledger::seal_pending(
            &author.chain,
            &author.identity,
            &author.store,
            &author.blocks
        )
        .await
        .unwrap(),
        Some(1)
    );
    leaf.wait_blocks(2).await;
    relay.wait_blocks(2).await;
    for node in [&leaf, &relay, &author] {
        let chain = node.chain.lock().await;
        assert_eq!(chain.pending_count(), 0);
        assert_eq!(chain.balance_of("Alice"), 70);
        assert_eq!(chain.balance_of("Bob"), 80);
        assert_eq!(chain.next_nonce("Alice"), 3);
        assert_eq!(chain.blocks[1].transactions.len(), 3);
        assert!(chain.validate_chain());
        let stored = node
            .store
            .load_or_create("validator", HashMap::new())
            .unwrap();
        assert_eq!(stored.blocks.len(), 2);
    }
    sleep(Duration::from_millis(200)).await;
    assert_eq!(author.chain.lock().await.blocks.len(), 2);

    // A cyclic topology now has two possible paths. The same transfer still
    // enters the validator's batch exactly once.
    leaf.connect_to(&author).await;
    let tx = Transaction::new("Alice".into(), "Bob".into(), 10, 3, &alice);
    let response = client
        .post("/transactions")
        .header(ContentType::JSON)
        .body(
            json!({
                "sender": "Alice", "receiver": "Bob", "amount": 10, "nonce": 3,
                "signature": hex::encode(tx.signature.unwrap().to_bytes())
            })
            .to_string(),
        )
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    author.wait_pending(1).await;
    assert_eq!(
        ledger::seal_pending(
            &author.chain,
            &author.identity,
            &author.store,
            &author.blocks
        )
        .await
        .unwrap(),
        Some(2)
    );
    leaf.wait_blocks(3).await;
    relay.wait_blocks(3).await;
    for node in [&leaf, &relay, &author] {
        let chain = node.chain.lock().await;
        assert_eq!(chain.pending_count(), 0);
        assert_eq!(chain.balance_of("Alice"), 60);
        assert_eq!(chain.balance_of("Bob"), 90);
        assert_eq!(chain.next_nonce("Alice"), 4);
        assert_eq!(chain.blocks[2].transactions.len(), 1);
    }

    // Lose the block notification deliberately. A receipt cannot suppress
    // gossip forever: the validator's Confirmed answer must trigger a sync.
    let tx = Transaction::new("Alice".into(), "Bob".into(), 10, 4, &alice);
    let response = client
        .post("/transactions")
        .header(ContentType::JSON)
        .body(
            json!({
                "sender": "Alice", "receiver": "Bob", "amount": 10, "nonce": 4,
                "signature": hex::encode(tx.signature.unwrap().to_bytes())
            })
            .to_string(),
        )
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    author.wait_pending(1).await;
    let (silent_announcer, _unread) = mpsc::channel(1);
    assert_eq!(
        ledger::seal_pending(
            &author.chain,
            &author.identity,
            &author.store,
            &silent_announcer
        )
        .await
        .unwrap(),
        Some(3)
    );
    leaf.wait_blocks(4).await;
    relay.wait_blocks(4).await;
    for node in [&leaf, &relay, &author] {
        let chain = node.chain.lock().await;
        assert_eq!(chain.pending_count(), 0);
        assert_eq!(chain.balance_of("Alice"), 50);
        assert_eq!(chain.balance_of("Bob"), 100);
        assert_eq!(chain.next_nonce("Alice"), 5);
        assert!(chain.validate_chain());
    }
}
