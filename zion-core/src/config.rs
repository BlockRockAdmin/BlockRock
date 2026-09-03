use dotenvy::dotenv;
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
}

impl Config {
    pub fn load() -> Result<Self, String> {
        dotenv().ok();
        let trongrid_api_key = env::var("TRONGRIDAPIKEY")
            .map_err(|e| format!("TRONGRIDAPIKEY environment variable not set: {}", e))?;
        let tron_address = env::var("TRON_ADDRESS")
            .map_err(|e| format!("TRON_ADDRESS environment variable not set: {}", e))?;
        Ok(Config {
            trongrid_api_key,
            tron_address,
            chain_path: path_or_default("CHAIN_PATH", DEFAULT_CHAIN_PATH),
            authority_name: env::var("AUTHORITY_NAME")
                .unwrap_or_else(|_| DEFAULT_AUTHORITY_NAME.to_string()),
            authority_key_path: path_or_default("AUTHORITY_KEY_PATH", DEFAULT_AUTHORITY_KEY_PATH),
        })
    }
}

fn path_or_default(variable: &str, default: &str) -> PathBuf {
    env::var(variable)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(default))
}
