//! Verifica che la porta gRPC faccia davvero le stesse cose della REST, con un
//! client vero contro un server vero.

use blockrock_core::transaction::Transaction;
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::{mpsc, Mutex};
use zion_core::api::grpc::blockrock::transaction_service_client::TransactionServiceClient;
use zion_core::api::grpc::blockrock::{
    AccountRequest, BalancesRequest, BlocksRequest, SubmitTransactionRequest, TransactionRequest,
};
use zion_core::api::grpc::{start_grpc, GrpcContext};
use zion_core::identity::NodeIdentity;
use zion_core::network::sync::BlockAnnouncement;
use zion_core::storage::ChainStore;

struct TestServer {
    endpoint: String,
    alice: SigningKey,
    _announcements: mpsc::Receiver<BlockAnnouncement>,
    _dir: TempDir,
}

/// Chiede al sistema una porta libera e la restituisce.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn start() -> TestServer {
    let dir = TempDir::new().unwrap();
    let store = ChainStore::new(dir.path().join("chain.json"));
    let identity =
        NodeIdentity::load_or_create("blockrock".to_string(), dir.path().join("authority.key"))
            .unwrap();

    let alice = SigningKey::generate(&mut OsRng);
    let mut chain = store.load_or_create(&identity.name, HashMap::new()).unwrap();
    chain.register_authority(&identity.name, identity.signing_key.verifying_key());
    chain.add_public_key("Alice", alice.verifying_key());
    store.save(&chain).unwrap();

    let (announcer, _announcements) = mpsc::channel::<BlockAnnouncement>(8);
    let port = free_port();
    let context = GrpcContext {
        blockchain: Arc::new(Mutex::new(chain)),
        identity: Arc::new(identity),
        store,
        announcer,
    };

    tokio::spawn(start_grpc(context, port, std::future::pending()));

    let endpoint = format!("http://127.0.0.1:{}", port);
    // Attende che il server sia in ascolto.
    for _ in 0..50 {
        if TransactionServiceClient::connect(endpoint.clone()).await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    TestServer {
        endpoint,
        alice,
        _announcements,
        _dir: dir,
    }
}

fn sign(key: &SigningKey, sender: &str, receiver: &str, amount: u64, nonce: u64) -> String {
    let payload =
        Transaction::unsigned(sender.to_string(), receiver.to_string(), amount, nonce)
            .signing_payload();
    hex::encode(key.sign(&payload).to_bytes())
}

#[tokio::test]
async fn a_transfer_submitted_over_grpc_lands_on_chain() {
    let server = start().await;
    let mut client = TransactionServiceClient::connect(server.endpoint.clone())
        .await
        .unwrap();

    let accepted = client
        .submit_transaction(SubmitTransactionRequest {
            sender: "Alice".to_string(),
            receiver: "Bob".to_string(),
            amount: 30,
            nonce: 0,
            signature: sign(&server.alice, "Alice", "Bob", 30, 0),
            payload: None,
        })
        .await
        .expect("il gRPC deve accettare il trasferimento")
        .into_inner();

    // Rileggibile dalla stessa porta.
    let found = client
        .get_transaction(TransactionRequest {
            id: accepted.id.clone(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(found.sender, "Alice");
    assert_eq!(found.recipient, "Bob");
    assert_eq!(found.amount, 30);

    // E i saldi si sono mossi.
    let balances = client
        .get_balances(BalancesRequest {})
        .await
        .unwrap()
        .into_inner()
        .balances;
    assert_eq!(balances.get("Alice"), Some(&70));
    assert_eq!(balances.get("Bob"), Some(&80));

    // Il nonce di Alice e' avanzato.
    let account = client
        .get_account(AccountRequest {
            name: "Alice".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(account.next_nonce, 1);
    assert!(account.public_key.is_some());

    // Il blocco e' in catena, col trasferimento dentro.
    let blocks = client
        .get_blocks(BlocksRequest {})
        .await
        .unwrap()
        .into_inner()
        .blocks;
    assert_eq!(blocks.len(), 2, "genesis piu' il blocco sigillato");
    assert_eq!(blocks[1].transactions.len(), 1);
    assert_eq!(blocks[1].transactions[0].id, accepted.id);
}

#[tokio::test]
async fn a_reading_anchored_over_grpc_keeps_its_payload() {
    let server = start().await;
    let mut client = TransactionServiceClient::connect(server.endpoint.clone())
        .await
        .unwrap();

    let payload = Transaction::unsigned_with_payload(
        "Alice".to_string(),
        "Alice".to_string(),
        0,
        0,
        Some("22.5".to_string()),
    )
    .signing_payload();

    let accepted = client
        .submit_transaction(SubmitTransactionRequest {
            sender: "Alice".to_string(),
            receiver: "Alice".to_string(),
            amount: 0,
            nonce: 0,
            signature: hex::encode(server.alice.sign(&payload).to_bytes()),
            payload: Some("22.5".to_string()),
        })
        .await
        .unwrap()
        .into_inner();

    let found = client
        .get_transaction(TransactionRequest { id: accepted.id })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(found.payload.as_deref(), Some("22.5"));
}

#[tokio::test]
async fn a_badly_signed_transfer_is_refused_with_invalid_argument() {
    let server = start().await;
    let mut client = TransactionServiceClient::connect(server.endpoint.clone())
        .await
        .unwrap();

    let intruso = SigningKey::generate(&mut OsRng);
    let status = client
        .submit_transaction(SubmitTransactionRequest {
            sender: "Alice".to_string(),
            receiver: "Bob".to_string(),
            amount: 30,
            nonce: 0,
            // Firmata da un'altra chiave.
            signature: sign(&intruso, "Alice", "Bob", 30, 0),
            payload: None,
        })
        .await
        .expect_err("una firma sbagliata non deve passare");

    assert_eq!(
        status.code(),
        tonic::Code::InvalidArgument,
        "colpa del chiamante, non del nodo: {}",
        status.message()
    );
}

#[tokio::test]
async fn minting_is_refused_on_the_grpc_door_too() {
    let server = start().await;
    let mut client = TransactionServiceClient::connect(server.endpoint.clone())
        .await
        .unwrap();

    let status = client
        .submit_transaction(SubmitTransactionRequest {
            sender: "System".to_string(),
            receiver: "Alice".to_string(),
            amount: 1000,
            nonce: 0,
            signature: sign(&server.alice, "System", "Alice", 1000, 0),
            payload: None,
        })
        .await
        .expect_err("il conio non passa da nessuna porta");

    assert_eq!(status.code(), tonic::Code::InvalidArgument);
    assert!(status.message().contains("mints value"));
}
