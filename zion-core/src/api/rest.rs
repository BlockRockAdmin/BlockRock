use blockrock_core::{
    block::Block,
    blockchain::{Blockchain, MINT_ACCOUNT},
    transaction::{Amount, Transaction},
};
use ed25519_dalek::{Signature, VerifyingKey};
use reqwest::Client;
use rocket::http::Status;
use rocket::response::stream::{Event, EventStream};
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};
use rocket::tokio::sync::broadcast::{error::RecvError, Sender};
use rocket::{get, post, State};
use serde_json::Value;
use std::fmt;
use std::fs;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::identity::NodeIdentity;
use crate::monitoring::{MetabolicStatus, SharedMetabolicStatus};
use crate::storage::ChainStore;

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

type ApiFailure = (Status, Json<ApiError>);

fn failure(status: Status, message: impl fmt::Display) -> ApiFailure {
    (
        status,
        Json(ApiError {
            error: message.to_string(),
        }),
    )
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
    pub block_index: u32,
}

/// Accepts a signed transfer, seals it into a block with this node's authority
/// key and persists the chain before answering.
///
/// One transaction per block: a mempool that batches pending transfers is the
/// natural next step, not a change of contract.
#[post("/transactions", format = "json", data = "<submission>")]
pub async fn submit_transaction(
    submission: Json<SubmitTransaction>,
    state: &State<Arc<Mutex<Blockchain>>>,
    identity: &State<NodeIdentity>,
    store: &State<ChainStore>,
) -> Result<Json<TransactionAccepted>, ApiFailure> {
    let submission = submission.into_inner();
    if submission.sender == MINT_ACCOUNT {
        return Err(bad_request(format!(
            "'{}' mints value and cannot be used from the API",
            MINT_ACCOUNT
        )));
    }

    let signature = decode_signature(&submission.signature)?;
    let transaction = Transaction::signed(
        submission.sender,
        submission.receiver,
        submission.amount,
        submission.nonce,
        signature,
    );
    let id = transaction.id.clone();

    let mut blockchain = state.lock().await;
    let block_index = blockchain
        .add_block(vec![transaction], &identity.name, &identity.signing_key)
        .map_err(bad_request)?;

    store.save(&blockchain).map_err(|error| {
        failure(
            Status::InternalServerError,
            format!("block {} accepted but not persisted: {}", block_index, error),
        )
    })?;

    Ok(Json(TransactionAccepted { id, block_index }))
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

#[get("/modules", format = "json")]
pub async fn get_modules() -> Json<String> {
    let yaml = fs::read_to_string("modules/blockchain/modules.yaml")
        .unwrap_or_else(|_| "Error: modules.yaml not found".to_string());
    Json(yaml)
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
