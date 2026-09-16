use dotenvy::dotenv;
use ed25519_dalek::VerifyingKey;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

/// Authority name used when a node creates the genesis block. It is a
/// network-wide value, not a per-node one: nodes that disagree on it end up
/// with different genesis hashes and will refuse to sync.
const DEFAULT_AUTHORITY_NAME: &str = "blockrock";
const DEFAULT_CHAIN_PATH: &str = "data/chain.json";
const DEFAULT_AUTHORITY_KEY_PATH: &str = "data/authority.key";

pub struct Config {
    pub trongrid_api_key: String,
    pub tron_address: String,
    /// Where the chain is persisted between restarts.
    pub chain_path: PathBuf,
    pub authority_name: String,
    /// Raw ed25519 seed this node seals blocks with, generated on first run.
    pub authority_key_path: PathBuf,
    /// Additional authorities to bootstrap at genesis, as "name:pubkey_hex"
    /// pairs. The local authority is always included automatically.
    pub initial_authorities: HashMap<String, VerifyingKey>,
}

impl Config {
    pub fn load() -> Result<Self, String> {
        dotenv().ok();
        let trongrid_api_key = env::var("TRONGRIDAPIKEY")
            .map_err(|e| format!("TRONGRIDAPIKEY environment variable not set: {}", e))?;
        let tron_address = env::var("TRON_ADDRESS")
            .map_err(|e| format!("TRON_ADDRESS environment variable not set: {}", e))?;
        let initial_authorities = parse_authorities(
            &env::var("INITIAL_AUTHORITIES").unwrap_or_default(),
        )?;
        Ok(Config {
            trongrid_api_key,
            tron_address,
            chain_path: path_or_default("CHAIN_PATH", DEFAULT_CHAIN_PATH),
            authority_name: env::var("AUTHORITY_NAME")
                .unwrap_or_else(|_| DEFAULT_AUTHORITY_NAME.to_string()),
            authority_key_path: path_or_default("AUTHORITY_KEY_PATH", DEFAULT_AUTHORITY_KEY_PATH),
            initial_authorities,
        })
    }
}

fn path_or_default(variable: &str, default: &str) -> PathBuf {
    env::var(variable)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(default))
}

/// Parses a comma-separated list of "name:pubkey_hex" pairs.
/// Example: "alice:abc123...,bob:def456..."
fn parse_authorities(raw: &str) -> Result<HashMap<String, VerifyingKey>, String> {
    let mut map = HashMap::new();
    for pair in raw.split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let (name, key_hex) = pair.split_once(':').ok_or_else(|| {
            format!(
                "INITIAL_AUTHORITIES: expected 'name:pubkey_hex', got '{}'",
                pair
            )
        })?;
        let bytes = hex::decode(key_hex.trim())
            .map_err(|e| format!("INITIAL_AUTHORITIES: invalid hex for '{}': {}", name, e))?;
        let bytes: [u8; 32] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| format!("INITIAL_AUTHORITIES: '{}' key must be 32 bytes", name))?;
        let key = VerifyingKey::from_bytes(&bytes)
            .map_err(|e| format!("INITIAL_AUTHORITIES: invalid key for '{}': {}", name, e))?;
        map.insert(name.to_string(), key);
    }
    Ok(map)
}
