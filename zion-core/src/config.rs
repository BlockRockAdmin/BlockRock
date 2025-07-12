use std::env;

pub struct Config {
    pub trongrid_api_key: String,
    pub tron_address: String,
}

impl Config {
    pub fn load() -> Result<Self, String> {
        // Stampa tutte le variabili d'ambiente per debug
        for (key, value) in env::vars() {
            println!("Env: {}={}", key, value);
        }
        let trongrid_api_key = env::var("TRONGRIDAPIKEY")
            .map_err(|e| format!("TRONGRIDAPIKEY environment variable not set: {}", e))?;
        let tron_address = env::var("TRON_ADDRESS")
            .map_err(|e| format!("TRON_ADDRESS environment variable not set: {}", e))?;
        println!("Loaded TRONGRIDAPIKEY: {}", trongrid_api_key);
        println!("Loaded TRON_ADDRESS: {}", tron_address);
        Ok(Config {
            trongrid_api_key,
            tron_address,
        })
    }
}
