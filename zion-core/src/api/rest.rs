use blockrock_core::{block::Block, blockchain::Blockchain};
use reqwest::Client;
use rocket::response::stream::{Event, EventStream};
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};
use rocket::tokio::sync::broadcast::{error::RecvError, Sender};
use rocket::{get, post, State};
use serde_json::Value;
use std::fs;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::monitoring::{MetabolicStatus, SharedMetabolicStatus};

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

#[get("/blocks", format = "json")]
pub async fn get_blocks(state: &State<Arc<Mutex<Blockchain>>>) -> Json<Vec<Block>> {
    let blockchain = state.lock().await;
    Json(blockchain.blocks.clone())
}

#[get("/balances", format = "json")]
pub async fn get_balances(state: &State<Arc<Mutex<Blockchain>>>) -> Json<Vec<(String, f64)>> {
    let blockchain = state.lock().await;
    let balances = blockchain.get_balances();
    Json(balances)
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
