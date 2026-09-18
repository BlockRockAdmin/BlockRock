use blockrock_core::{
    block::Block,
    blockchain::{Blockchain, MINT_ACCOUNT},
    transaction::{Amount, Transaction},
};
use ed25519_dalek::{Signature, VerifyingKey};
use reqwest::Client;
use rocket::http::{Header, Status};
use rocket::response::stream::{Event, EventStream};
use rocket::serde::json::Json;
use rocket::response::Responder;
use rocket::serde::{Deserialize, Serialize};
use rocket::tokio::sync::broadcast::{error::RecvError, Sender};
use rocket::{get, post, Request, State};
use serde_json::Value;
use std::fmt;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::identity::NodeIdentity;
use crate::ledger::{self, SubmitError};
use crate::monitoring::{MetabolicStatus, SharedMetabolicStatus};
use crate::network::sync::BlockAnnouncement;
use crate::storage::ChainStore;

/// Blocks sealed by this node are handed to the P2P loop, which pushes them to
/// every connected peer. Announcing is best effort: a full or closed channel
/// never fails a transaction that is already on chain.
pub type BlockAnnouncer = mpsc::Sender<BlockAnnouncement>;

/// Reusable connection pool and immutable Tron configuration.
/// Constructing this once avoids DNS/TLS setup and environment parsing per request.
pub struct TronService {
    client: Client,
    api_key: String,
    address: String,
}

impl TronService {
    pub fn new(api_key: String, address: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            address,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
}

/// Un errore dell'API. Porta un `Retry-After` solo quando riprovare ha senso:
/// e' il segnale con cui un client distingue il backoff dalla rinuncia.
pub struct ApiFailure {
    status: Status,
    body: ApiError,
    retry_after: Option<Header<'static>>,
}

impl<'r> Responder<'r, 'static> for ApiFailure {
    fn respond_to(self, request: &'r Request<'_>) -> rocket::response::Result<'static> {
        let mut response = (self.status, Json(self.body)).respond_to(request)?;
        if let Some(header) = self.retry_after {
            response.set_header(header);
        }
        Ok(response)
    }
}

fn failure(status: Status, message: impl fmt::Display) -> ApiFailure {
    ApiFailure {
        status,
        body: ApiError {
            error: message.to_string(),
        },
        retry_after: None,
    }
}

/// Il nodo e' pieno: 429 con l'attesa suggerita, che e' il tempo entro cui il
/// prossimo blocco svuotera' il mempool.
fn too_many_requests(message: impl fmt::Display) -> ApiFailure {
    let seconds = ledger::BLOCK_TIME.as_secs().max(1);
    ApiFailure {
        status: Status::TooManyRequests,
        body: ApiError {
            error: message.to_string(),
        },
        retry_after: Some(Header::new("Retry-After", seconds.to_string())),
    }
}

fn bad_request(message: impl fmt::Display) -> ApiFailure {
    failure(Status::BadRequest, message)
}

#[get("/blocks", format = "json")]
pub async fn get_blocks(state: &State<Arc<Mutex<Blockchain>>>) -> Json<Vec<Block>> {
    let blockchain = state.lock().await;
    Json(blockchain.blocks.clone())
}

#[get("/balances", format = "json")]
pub async fn get_balances(state: &State<Arc<Mutex<Blockchain>>>) -> Json<Vec<(String, Amount)>> {
    let blockchain = state.lock().await;
    let balances = blockchain.get_balances();
    Json(balances)
}

/// A transaction signed by its sender, as submitted by a client.
#[derive(Debug, Deserialize)]
pub struct SubmitTransaction {
    pub sender: String,
    pub receiver: String,
    /// Amount in the smallest indivisible unit.
    pub amount: Amount,
    /// Must equal the sender's next nonce, available from `GET /accounts/<name>`.
    pub nonce: u64,
    /// Hex-encoded ed25519 signature over the canonical transaction payload.
    pub signature: String,
}

#[derive(Debug, Serialize)]
pub struct TransactionAccepted {
    pub id: String,
    /// Number of transactions currently waiting in the mempool (including this
    /// one). The transaction will be sealed into a block on the next block-time
    /// tick.
    pub pending: usize,
}

/// Accepts a signed transfer and queues it in the mempool. The transaction is
/// **not** immediately sealed into a block; the node's block-time loop batches
/// pending transactions periodically.
#[post("/transactions", format = "json", data = "<submission>")]
pub async fn submit_transaction(
    submission: Json<SubmitTransaction>,
    state: &State<Arc<Mutex<Blockchain>>>,
    identity: &State<NodeIdentity>,
    store: &State<ChainStore>,
    announcer: &State<BlockAnnouncer>,
) -> Result<Json<TransactionAccepted>, ApiFailure> {
    let submission = submission.into_inner();
    let signature = decode_signature(&submission.signature)?;
    let transaction = Transaction::signed(
        submission.sender,
        submission.receiver,
        submission.amount,
        submission.nonce,
        signature,
    );
    let id = transaction.id.clone();
    let pending = queue_and_maybe_seal(transaction, state, identity, store, announcer).await?;
    Ok(Json(TransactionAccepted { id, pending }))
}

/// Traduce l'esito dell'invio condiviso nei codici HTTP: una richiesta
/// malfatta e' 400, un guasto del nodo e' 500.
async fn queue_and_maybe_seal(
    transaction: Transaction,
    state: &State<Arc<Mutex<Blockchain>>>,
    identity: &State<NodeIdentity>,
    store: &State<ChainStore>,
    announcer: &State<BlockAnnouncer>,
) -> Result<usize, ApiFailure> {
    match ledger::submit(transaction, state, identity, store, announcer).await {
        Ok(queued) => Ok(queued.pending),
        Err(SubmitError::Rejected(message)) => Err(bad_request(message)),
        Err(SubmitError::Busy(message)) => Err(too_many_requests(message)),
        Err(SubmitError::Internal(message)) => Err(failure(Status::InternalServerError, message)),
    }
}

#[derive(Debug, Deserialize)]
pub struct RegisterAccount {
    pub name: String,
    /// Hex-encoded ed25519 public key, 32 bytes.
    pub public_key: String,
}

#[derive(Debug, Serialize)]
pub struct AccountState {
    pub name: String,
    pub balance: Amount,
    /// Nonce the next transaction from this account must carry.
    pub next_nonce: u64,
    pub public_key: Option<String>,
}

/// Registers the public key transactions from `name` are verified against.
/// First registration wins: rebinding an existing name is refused.
#[post("/accounts", format = "json", data = "<registration>")]
pub async fn register_account(
    registration: Json<RegisterAccount>,
    state: &State<Arc<Mutex<Blockchain>>>,
    store: &State<ChainStore>,
) -> Result<Json<AccountState>, ApiFailure> {
    let registration = registration.into_inner();
    if registration.name == MINT_ACCOUNT {
        return Err(bad_request(format!(
            "'{}' is reserved by the node",
            MINT_ACCOUNT
        )));
    }
    let public_key = decode_public_key(&registration.public_key)?;

    let mut blockchain = state.lock().await;
    if let Some(existing) = blockchain.public_key(&registration.name) {
        if existing != &public_key {
            return Err(failure(
                Status::Conflict,
                format!("'{}' is already bound to another key", registration.name),
            ));
        }
    }

    blockchain.add_public_key(&registration.name, public_key);
    store.save(&blockchain).map_err(|error| {
        failure(
            Status::InternalServerError,
            format!("account registered but not persisted: {}", error),
        )
    })?;

    Ok(Json(account_state(&blockchain, registration.name)))
}

/// Balance and next nonce, everything a client needs to build a transaction.
#[get("/accounts/<name>", format = "json")]
pub async fn get_account(
    name: &str,
    state: &State<Arc<Mutex<Blockchain>>>,
) -> Result<Json<AccountState>, ApiFailure> {
    let blockchain = state.lock().await;
    if blockchain.public_key(name).is_none() && !blockchain.balances.contains_key(name) {
        return Err(failure(Status::NotFound, format!("unknown account '{}'", name)));
    }
    Ok(Json(account_state(&blockchain, name.to_string())))
}

fn account_state(blockchain: &Blockchain, name: String) -> AccountState {
    AccountState {
        balance: blockchain.balance_of(&name),
        next_nonce: blockchain.next_nonce(&name),
        public_key: blockchain
            .public_key(&name)
            .map(|key| hex::encode(key.to_bytes())),
        name,
    }
}

fn decode_signature(signature: &str) -> Result<Signature, ApiFailure> {
    let bytes = hex::decode(signature.trim())
        .map_err(|error| bad_request(format!("signature is not valid hex: {}", error)))?;
    let bytes: [u8; 64] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| bad_request("signature must be 64 bytes"))?;
    Ok(Signature::from_bytes(&bytes))
}

fn decode_public_key(public_key: &str) -> Result<VerifyingKey, ApiFailure> {
    let bytes = hex::decode(public_key.trim())
        .map_err(|error| bad_request(format!("public key is not valid hex: {}", error)))?;
    let bytes: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| bad_request("public key must be 32 bytes"))?;
    VerifyingKey::from_bytes(&bytes)
        .map_err(|error| bad_request(format!("invalid ed25519 public key: {}", error)))
}

#[get("/tron_balance", format = "json")]
pub async fn tron_balance(service: &State<TronService>) -> Result<Json<f64>, String> {
    let response = service
        .client
        .post("https://nile.trongrid.io/wallet/getaccount")
        .header("TRON-PRO-API-KEY", &service.api_key)
        .json(&serde_json::json!({
            "address": service.address,
            "visible": true
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let data: Value = response.json().await.map_err(|e| e.to_string())?;
    let balance = data["balance"].as_f64().unwrap_or(0.0) / 1_000_000.0;
    Ok(Json(balance))
}

#[get("/health")]
pub async fn health() -> &'static str {
    "OK"
}

/// Latest policy decision from the autonomous monitoring loop.
/// This endpoint is informational: it never performs infrastructure changes.
#[get("/metabolism", format = "json")]
pub async fn metabolism_status(state: &State<SharedMetabolicStatus>) -> Json<MetabolicStatus> {
    Json(state.read().await.clone())
}

/// Il manifesto descrive i moduli compilati dentro questo binario, quindi
/// viaggia con lui: incluso a compile time e risolto rispetto al crate, non
/// alla working directory. Letto da disco falliva a seconda di come il nodo
/// veniva avviato, e restituiva un 200 con dentro la stringa d'errore.
const MODULES_MANIFEST: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/modules/blockchain/modules.yaml"));

#[get("/modules", format = "json")]
pub async fn get_modules() -> Json<String> {
    Json(MODULES_MANIFEST.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReading {
    pub sensor_id: String,
    pub value: f64,
}

#[post("/sensors", format = "json", data = "<reading>")]
pub async fn post_sensor(
    reading: Json<SensorReading>,
    tx: &State<Sender<SensorReading>>,
) -> &'static str {
    let _ = tx.send(reading.into_inner());
    "OK"
}

#[derive(Debug, Deserialize)]
pub struct CommitReading {
    /// Conto del sensore. Deve avere una chiave pubblica registrata con
    /// `POST /accounts`: una lettura non firmata da una chiave nota non entra
    /// in catena, altrimenti chiunque potrebbe inventare misure.
    pub sensor_id: String,
    /// Il valore esattamente com'è stato firmato. Viaggia come stringa perché
    /// la firma copre questi byte: riformattare un numero (`1.0` contro `1`, o
    /// una precisione diversa) darebbe byte diversi e firma invalida.
    pub value: String,
    /// Prossimo nonce del sensore, da `GET /accounts/<sensor_id>`.
    pub nonce: u64,
    /// Firma ed25519 in hex sul payload canonico della transazione.
    pub signature: String,
}

#[derive(Debug, Serialize)]
pub struct ReadingCommitted {
    /// Id della transazione che porta la lettura.
    pub id: String,
    pub pending: usize,
}

/// Ancora una lettura firmata alla catena.
///
/// `POST /sensors` è telemetria viva: arriva, viene ritrasmessa sullo stream e
/// sparisce. Questo endpoint è l'altra metà: la lettura diventa una transazione
/// a importo zero dal sensore verso sé stesso, quindi non muove valore ma resta
/// in catena firmata e non alterabile. Chi la legge può verificarla con la
/// chiave pubblica del sensore.
#[post("/sensors/commit", format = "json", data = "<reading>")]
pub async fn commit_sensor_reading(
    reading: Json<CommitReading>,
    state: &State<Arc<Mutex<Blockchain>>>,
    identity: &State<NodeIdentity>,
    store: &State<ChainStore>,
    announcer: &State<BlockAnnouncer>,
    sensor_tx: &State<Sender<SensorReading>>,
) -> Result<Json<ReadingCommitted>, ApiFailure> {
    let reading = reading.into_inner();
    let signature = decode_signature(&reading.signature)?;
    // Mittente e destinatario coincidono e l'importo è zero: la transazione
    // esiste solo per portare il dato, non per spostare fondi.
    let transaction = Transaction::signed_with_payload(
        reading.sensor_id.clone(),
        reading.sensor_id.clone(),
        0,
        reading.nonce,
        Some(reading.value.clone()),
        signature,
    );
    let id = transaction.id.clone();
    let pending = queue_and_maybe_seal(transaction, state, identity, store, announcer).await?;

    // Chi guarda lo stream vede anche le letture ancorate, purché siano
    // numeriche: lo stream trasporta un f64, non una stringa qualunque.
    if let Ok(value) = reading.value.parse::<f64>() {
        let _ = sensor_tx.send(SensorReading {
            sensor_id: reading.sensor_id,
            value,
        });
    }

    Ok(Json(ReadingCommitted { id, pending }))
}

#[get("/sensors/stream")]
pub async fn sensor_events(tx: &State<Sender<SensorReading>>) -> EventStream![Event + '_] {
    let mut rx = tx.subscribe();
    EventStream! {
        loop {
            let msg = rx.recv().await;
            match msg {
                Ok(reading) => yield Event::json(&reading),
                Err(RecvError::Closed) => break,
                Err(RecvError::Lagged(_)) => continue,
            }
        }
    }
}
