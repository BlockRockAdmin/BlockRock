use rocket::{get, State};
use rocket::serde::json::Json;
use blockrock_core::{block::Block, blockchain::Blockchain};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::fs;
use reqwest::Client;
use serde_json::Value;

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
pub async fn tron_balance() -> Json<f64> {
    let config = crate::config::Config::load().expect("Failed to load config");
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
        .expect("Failed to call TronGrid");
    
    let data: Value = response.json().await.expect("Failed to parse JSON");
    let balance = data["balance"].as_f64().unwrap_or(0.0) / 1_000_000.0; // Converti da Sun a TRX
    Json(balance)
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
