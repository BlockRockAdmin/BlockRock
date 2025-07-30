use std::env;
use dotenvy::dotenv;

pub struct Config {
    pub trongrid_api_key: String,
    pub tron_address: String,
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
        })
    }
}
