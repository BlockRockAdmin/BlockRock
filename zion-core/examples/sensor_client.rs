//! Un sensore che firma le sue letture e le ancora alla catena.
//!
//! Serve a due cose: mostrare cosa deve fare un client IoT vero, e verificare
//! sul nodo in esecuzione che piu' letture ravvicinate finiscano in un blocco
//! solo invece che in uno ciascuna.
//!
//! ```bash
//! cargo run --example sensor_client              # contro localhost:8000
//! cargo run --example sensor_client -- http://altro-nodo:8000
//! ```

use blockrock_core::transaction::Transaction;
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use serde_json::Value;
use std::time::Duration;

const SENSOR: &str = "termometro-demo";
const READINGS: usize = 5;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://localhost:8000".to_string());
    let http = reqwest::Client::new();

    // Un sensore e' un conto come un altro: prima registra la sua chiave
    // pubblica, poi firma con quella privata.
    let key = SigningKey::generate(&mut OsRng);
    let registered = http
        .post(format!("{}/accounts", base))
        .json(&serde_json::json!({
            "name": SENSOR,
            "public_key": hex::encode(key.verifying_key().to_bytes()),
        }))
        .send()
        .await?;
    println!("registrazione sensore: {}", registered.status());

    let blocks_before = block_count(&http, &base).await?;

    // Le letture partono una dietro l'altra, senza aspettare un blocco fra
    // l'una e l'altra: e' il caso che prima produceva un blocco per lettura.
    for nonce in 0..READINGS as u64 {
        let value = format!("{:.1}", 20.0 + nonce as f64 * 0.5);
        let signature = key.sign(
            &Transaction::unsigned_with_payload(
                SENSOR.to_string(),
                SENSOR.to_string(),
                0,
                nonce,
                Some(value.clone()),
            )
            .signing_payload(),
        );

        let response = http
            .post(format!("{}/sensors/commit", base))
            .json(&serde_json::json!({
                "sensor_id": SENSOR,
                "value": value,
                "nonce": nonce,
                "signature": hex::encode(signature.to_bytes()),
            }))
            .send()
            .await?;

        let status = response.status();
        let body: Value = response.json().await.unwrap_or(Value::Null);
        println!(
            "lettura {} = {}: {} (in attesa: {})",
            nonce,
            value,
            status,
            body.get("pending").and_then(Value::as_u64).unwrap_or(0)
        );
    }

    println!("\nattendo il tick del nodo...");
    tokio::time::sleep(Duration::from_secs(7)).await;

    let blocks_after = block_count(&http, &base).await?;
    let new_blocks = blocks_after.saturating_sub(blocks_before);
    println!(
        "{} letture hanno prodotto {} blocco/i",
        READINGS, new_blocks
    );
    if new_blocks <= 1 {
        println!("il batching funziona: un lotto, un blocco.");
    } else {
        println!("attenzione: ci si aspettava un blocco solo.");
    }

    Ok(())
}

async fn block_count(http: &reqwest::Client, base: &str) -> Result<usize, Box<dyn std::error::Error>> {
    let blocks: Value = http.get(format!("{}/blocks", base)).send().await?.json().await?;
    Ok(blocks.as_array().map_or(0, |b| b.len()))
}
