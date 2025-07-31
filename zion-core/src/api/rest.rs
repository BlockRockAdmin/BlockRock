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
pub async fn tron_balance() -> Result<Json<f64>, String> {
    let config = crate::config::Config::load().map_err(|e| e)?;
    let client = Client::new();
    let response = client
        .post("https://nile.trongrid.io/wallet/getaccount")
        .header("TRON-PRO-API-KEY", &config.trongrid_api_key)
        .json(&serde_json::json!({
            "address": config.tron_address,
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
