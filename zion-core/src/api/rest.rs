use blockrock_core::{block::Block, blockchain::Blockchain};
use reqwest::Client;
use rocket::serde::json::Json;
use rocket::{get, State};
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
