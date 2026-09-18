use blockrock_core::blockchain::Blockchain;
use blockrock_core::transaction::Transaction;
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use rocket::tokio::sync::broadcast;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::{mpsc, Mutex};
use zion_core::api::rest::{SensorReading, TronService};
use zion_core::identity::NodeIdentity;
use zion_core::ledger;
use zion_core::monitoring::shared_metabolic_status;
use zion_core::network::sync::BlockAnnouncement;
use zion_core::server::{self, ServerContext};
use zion_core::storage::ChainStore;

struct TestNode {
    client: Client,
    store: ChainStore,
    /// What the node would push to its peers.
    announcements: mpsc::Receiver<BlockAnnouncement>,
    /// Quanto serve per chiudere un blocco a mano: nei test non gira il tick
    /// periodico, e dalla soglia in poi un invio da solo non sigilla piu'.
    blockchain: Arc<Mutex<Blockchain>>,
    identity: Arc<NodeIdentity>,
    announcer: mpsc::Sender<BlockAnnouncement>,
    _dir: TempDir,
}

impl TestNode {
    /// Chiude un blocco esattamente come farebbe il tick del nodo.
    async fn seal(&self) {
        ledger::seal_pending(
            &self.blockchain,
            &self.identity,
            &self.store,
            &self.announcer,
        )
        .await
        .expect("la sigillatura non deve fallire");
    }
}

async fn start_node() -> TestNode {
    let dir = TempDir::new().unwrap();
    let store = ChainStore::new(dir.path().join("chain.json"));
    let identity =
        NodeIdentity::load_or_create("blockrock".to_string(), dir.path().join("authority.key"))
            .unwrap();

    let mut chain = store.load_or_create(&identity.name, HashMap::new()).unwrap();
    chain.register_authority(&identity.name, identity.signing_key.verifying_key());
    store.save(&chain).unwrap();

    let (sensor_tx, _sensor_rx) = broadcast::channel::<SensorReading>(8);
    let (block_announcer, announcements) = mpsc::channel::<BlockAnnouncement>(8);
    let blockchain = Arc::new(Mutex::new(chain));
    let identity = Arc::new(identity);
    let rocket = server::build(ServerContext {
        blockchain: Arc::clone(&blockchain),
        identity: (*identity).clone(),
        store: store.clone(),
        sensor_tx,
        metabolic_status: shared_metabolic_status(),
        tron_service: TronService::new("test-key".to_string(), "test-address".to_string()),
        block_announcer: block_announcer.clone(),
    });

    TestNode {
        client: Client::tracked(rocket).await.unwrap(),
        store,
        announcements,
        blockchain,
        identity,
        announcer: block_announcer,
        _dir: dir,
    }
}

fn sign(key: &SigningKey, sender: &str, receiver: &str, amount: u64, nonce: u64) -> String {
    let payload =
        Transaction::unsigned(sender.to_string(), receiver.to_string(), amount, nonce).signing_payload();
    hex::encode(key.sign(&payload).to_bytes())
}

async fn register(client: &Client, name: &str, key: &SigningKey) -> Status {
    client
        .post("/accounts")
        .header(ContentType::JSON)
        .body(
            json!({
                "name": name,
                "public_key": hex::encode(key.verifying_key().to_bytes()),
            })
            .to_string(),
        )
        .dispatch()
        .await
        .status()
}

async fn submit(client: &Client, body: Value) -> (Status, Value) {
    let response = client
        .post("/transactions")
        .header(ContentType::JSON)
        .body(body.to_string())
        .dispatch()
        .await;
    let status = response.status();
    let body: Value = serde_json::from_str(&response.into_string().await.unwrap()).unwrap();
    (status, body)
}

#[rocket::async_test]
async fn a_signed_transfer_is_sealed_and_survives_a_restart() {
    let mut node = start_node().await;
    let alice = SigningKey::generate(&mut OsRng);
    assert_eq!(register(&node.client, "Alice", &alice).await, Status::Ok);

    let account: Value = serde_json::from_str(
        &node
            .client
            .get("/accounts/Alice")
            .dispatch()
            .await
            .into_string()
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(account["balance"], 100);
    assert_eq!(account["next_nonce"], 0);

    let (status, accepted) = submit(
        &node.client,
        json!({
            "sender": "Alice",
            "receiver": "Bob",
            "amount": 30,
            "nonce": 0,
            "signature": sign(&alice, "Alice", "Bob", 30, 0),
        }),
    )
    .await;
    assert_eq!(status, Status::Ok);
    // La transazione e' accettata ma non ancora sigillata: sotto la soglia
    // resta nel mempool finche' non passa il tick.
    assert_eq!(accepted["pending"].as_u64().unwrap(), 1);
    let before_tick = Blockchain::load_from_file(node.store.path()).unwrap();
    assert_eq!(
        before_tick.blocks.len(),
        1,
        "prima del tick in catena c'e' solo il genesis"
    );

    node.seal().await;

    let balances: Value = serde_json::from_str(
        &node
            .client
            .get("/balances")
            .dispatch()
            .await
            .into_string()
            .await
            .unwrap(),
    )
    .unwrap();
    let balances = balances.as_array().unwrap();
    assert!(balances.contains(&json!(["Alice", 70])));
    assert!(balances.contains(&json!(["Bob", 80])));

    // The sealed block is handed to the P2P loop for the peers.
    let announced = node.announcements.try_recv().expect("the block is announced");
    assert_eq!(announced.block.index, 1);
    assert!(announced.accounts.contains_key("Alice"));

    // The chain was written through: a restart resumes from the same tip.
    let reloaded = Blockchain::load_from_file(node.store.path()).unwrap();
    assert_eq!(reloaded.blocks.len(), 2);
    assert!(reloaded.validate_chain());
    assert_eq!(reloaded.balance_of("Alice"), 70);
    assert_eq!(
        reloaded.get_transaction(accepted["id"].as_str().unwrap()).unwrap().amount,
        30
    );
}

#[rocket::async_test]
async fn a_replayed_or_unaffordable_transfer_is_refused() {
    let node = start_node().await;
    let alice = SigningKey::generate(&mut OsRng);
    register(&node.client, "Alice", &alice).await;

    let transfer = json!({
        "sender": "Alice",
        "receiver": "Bob",
        "amount": 10,
        "nonce": 0,
        "signature": sign(&alice, "Alice", "Bob", 10, 0),
    });
    assert_eq!(submit(&node.client, transfer.clone()).await.0, Status::Ok);

    // Gli stessi byte firmati di nuovo: ora e' un duplicato gia' in attesa nel
    // mempool. Prima della soglia lo stesso invio falliva perche' il nonce era
    // gia' stato consumato in catena: due strade diverse, stesso rifiuto.
    assert_eq!(submit(&node.client, transfer).await.0, Status::BadRequest);

    // Sigillato il primo trasferimento, il nonce avanza e si puo' provare a
    // spendere piu' di quanto resta.
    node.seal().await;

    let (status, error) = submit(
        &node.client,
        json!({
            "sender": "Alice",
            "receiver": "Bob",
            "amount": 1_000,
            "nonce": 1,
            "signature": sign(&alice, "Alice", "Bob", 1_000, 1),
        }),
    )
    .await;
    assert_eq!(status, Status::BadRequest);
    assert!(error["error"].as_str().unwrap().contains("tried to send"));

    // Minting is not reachable from the API.
    let (status, _) = submit(
        &node.client,
        json!({
            "sender": "System",
            "receiver": "Bob",
            "amount": 5,
            "nonce": 0,
            "signature": sign(&alice, "System", "Bob", 5, 0),
        }),
    )
    .await;
    assert_eq!(status, Status::BadRequest);
}

#[rocket::async_test]
async fn a_transfer_signed_by_the_wrong_key_is_refused() {
    let node = start_node().await;
    let alice = SigningKey::generate(&mut OsRng);
    let impostor = SigningKey::generate(&mut OsRng);
    register(&node.client, "Alice", &alice).await;

    let (status, _) = submit(
        &node.client,
        json!({
            "sender": "Alice",
            "receiver": "Bob",
            "amount": 10,
            "nonce": 0,
            "signature": sign(&impostor, "Alice", "Bob", 10, 0),
        }),
    )
    .await;
    assert_eq!(status, Status::BadRequest);

    // Rebinding a registered name to another key is refused too.
    assert_eq!(
        register(&node.client, "Alice", &impostor).await,
        Status::Conflict
    );
}

#[rocket::async_test]
async fn an_unknown_account_is_reported_as_missing() {
    let node = start_node().await;
    let response = node.client.get("/accounts/Nobody").dispatch().await;
    assert_eq!(response.status(), Status::NotFound);
}

// --- Letture ancorate ------------------------------------------------------

/// Firma una lettura esattamente come deve farlo un sensore: la transazione è
/// da lui verso sé stesso, importo zero, e il valore viaggia nel payload.
fn sign_reading(key: &SigningKey, sensor_id: &str, value: &str, nonce: u64) -> String {
    let payload = Transaction::unsigned_with_payload(
        sensor_id.to_string(),
        sensor_id.to_string(),
        0,
        nonce,
        Some(value.to_string()),
    )
    .signing_payload();
    hex::encode(key.sign(&payload).to_bytes())
}

async fn commit_reading(client: &Client, body: Value) -> (Status, Value) {
    let response = client
        .post("/sensors/commit")
        .header(ContentType::JSON)
        .body(body.to_string())
        .dispatch()
        .await;
    let status = response.status();
    let body: Value = serde_json::from_str(&response.into_string().await.unwrap()).unwrap();
    (status, body)
}

#[rocket::async_test]
async fn a_signed_reading_is_anchored_and_survives_a_restart() {
    let node = start_node().await;
    let sensor = SigningKey::generate(&mut OsRng);
    assert_eq!(register(&node.client, "termometro", &sensor).await, Status::Ok);

    let (status, accepted) = commit_reading(
        &node.client,
        json!({
            "sensor_id": "termometro",
            "value": "22.5",
            "nonce": 0,
            "signature": sign_reading(&sensor, "termometro", "22.5", 0),
        }),
    )
    .await;
    assert_eq!(status, Status::Ok);
    let id = accepted["id"].as_str().unwrap().to_string();

    node.seal().await;

    // La lettura sopravvive a un riavvio, perche' e' in catena su disco.
    let reloaded = Blockchain::load_from_file(node.store.path()).unwrap();
    let stored = reloaded
        .get_transaction(&id)
        .expect("la lettura deve essere in catena dopo il riavvio");
    assert_eq!(stored.payload.as_deref(), Some("22.5"));
    assert_eq!(stored.amount, 0, "una lettura non muove valore");
}

#[rocket::async_test]
async fn a_reading_signed_over_a_different_value_is_refused() {
    let node = start_node().await;
    let sensor = SigningKey::generate(&mut OsRng);
    assert_eq!(register(&node.client, "termometro", &sensor).await, Status::Ok);

    // Il sensore firma 22.5, ma sulla rete viaggia 35.0.
    let (status, _) = commit_reading(
        &node.client,
        json!({
            "sensor_id": "termometro",
            "value": "35.0",
            "nonce": 0,
            "signature": sign_reading(&sensor, "termometro", "22.5", 0),
        }),
    )
    .await;
    assert_eq!(status, Status::BadRequest);
}

#[rocket::async_test]
async fn a_reading_from_an_unregistered_sensor_is_refused() {
    let node = start_node().await;
    let sconosciuto = SigningKey::generate(&mut OsRng);

    let (status, _) = commit_reading(
        &node.client,
        json!({
            "sensor_id": "intruso",
            "value": "22.5",
            "nonce": 0,
            "signature": sign_reading(&sconosciuto, "intruso", "22.5", 0),
        }),
    )
    .await;
    assert_eq!(
        status,
        Status::BadRequest,
        "senza chiave pubblica registrata la lettura non entra"
    );
}

#[rocket::async_test]
async fn a_replayed_reading_is_refused() {
    let node = start_node().await;
    let sensor = SigningKey::generate(&mut OsRng);
    assert_eq!(register(&node.client, "termometro", &sensor).await, Status::Ok);

    let body = json!({
        "sensor_id": "termometro",
        "value": "22.5",
        "nonce": 0,
        "signature": sign_reading(&sensor, "termometro", "22.5", 0),
    });

    let (first, _) = commit_reading(&node.client, body.clone()).await;
    assert_eq!(first, Status::Ok);

    // Stesso nonce: e' un replay.
    let (second, _) = commit_reading(&node.client, body).await;
    assert_eq!(second, Status::BadRequest);
}

// --- Saturazione: cosa vede il client ---------------------------------------

/// Riempie il mempool fino al suo tetto, senza passare dall'API: qui interessa
/// come il nodo risponde quando e' pieno, non quanto ci mette a riempirsi.
async fn fill_mempool(node: &TestNode) {
    let filler = "x".repeat(blockrock_core::transaction::MAX_PAYLOAD_BYTES);
    let mut chain = node.blockchain.lock().await;
    let mut i = 0;
    loop {
        let name = format!("riempitivo{}", i);
        let key = SigningKey::generate(&mut OsRng);
        chain.add_public_key(&name, key.verifying_key());
        let tx = Transaction::new_with_payload(
            name.clone(),
            name,
            0,
            0,
            Some(filler.clone()),
            &key,
        );
        if chain.queue_transaction(tx).is_err() {
            break;
        }
        i += 1;
        assert!(i < 10_000, "il mempool non si riempie");
    }
}

/// Una lettura al massimo della taglia consentita: dopo che il pool e' pieno di
/// payload grossi, il margine rimasto non basta piu' per una di queste.
fn big_reading(key: &SigningKey, sensor: &str, nonce: u64) -> Value {
    let value = "x".repeat(blockrock_core::transaction::MAX_PAYLOAD_BYTES);
    let signature = key.sign(
        &Transaction::unsigned_with_payload(
            sensor.to_string(),
            sensor.to_string(),
            0,
            nonce,
            Some(value.clone()),
        )
        .signing_payload(),
    );
    json!({
        "sensor_id": sensor,
        "value": value,
        "nonce": nonce,
        "signature": hex::encode(signature.to_bytes()),
    })
}

#[rocket::async_test]
async fn a_saturated_node_asks_the_client_to_back_off() {
    let node = start_node().await;
    let sensor = SigningKey::generate(&mut OsRng);
    register(&node.client, "termometro", &sensor).await;
    fill_mempool(&node).await;

    let response = node
        .client
        .post("/sensors/commit")
        .header(ContentType::JSON)
        .body(big_reading(&sensor, "termometro", 0).to_string())
        .dispatch()
        .await;

    // 429, non 400: la lettura e' valida, e' il nodo a essere pieno. Un 400
    // direbbe al client di non riprovare, che e' l'opposto di quel che serve.
    assert_eq!(response.status(), Status::TooManyRequests);

    let retry_after = response
        .headers()
        .get_one("Retry-After")
        .expect("un 429 senza Retry-After non dice al client quanto aspettare");
    let seconds: u64 = retry_after.parse().expect("Retry-After in secondi");
    assert!(seconds >= 1, "attesa suggerita non sensata: {}", seconds);

    let body: Value = serde_json::from_str(&response.into_string().await.unwrap()).unwrap();
    assert!(
        body["error"].as_str().unwrap().contains("mempool is full"),
        "il motivo deve essere esplicito: {}",
        body["error"]
    );
}

#[rocket::async_test]
async fn a_small_transaction_still_fits_while_the_pool_is_nearly_full() {
    // Il tetto e' sui byte, non sul numero: finche' c'e' margine una
    // transazione piccola deve passare anche con il pool quasi pieno.
    let node = start_node().await;
    let alice = SigningKey::generate(&mut OsRng);
    register(&node.client, "Alice", &alice).await;
    fill_mempool(&node).await;

    let (status, _) = submit(
        &node.client,
        json!({
            "sender": "Alice",
            "receiver": "Bob",
            "amount": 10,
            "nonce": 0,
            "signature": sign(&alice, "Alice", "Bob", 10, 0),
        }),
    )
    .await;
    assert_eq!(status, Status::Ok);
}

#[rocket::async_test]
async fn the_same_reading_is_accepted_once_the_block_has_been_sealed() {
    let node = start_node().await;
    let sensor = SigningKey::generate(&mut OsRng);
    register(&node.client, "termometro", &sensor).await;
    fill_mempool(&node).await;

    let reading = big_reading(&sensor, "termometro", 0);
    let (status, _) = commit_reading(&node.client, reading.clone()).await;
    assert_eq!(status, Status::TooManyRequests);

    // Il backoff ha senso solo se l'attesa serve davvero: dopo il blocco la
    // stessa identica lettura deve passare.
    node.seal().await;

    let (status, _) = commit_reading(&node.client, reading).await;
    assert_eq!(status, Status::Ok);
}
