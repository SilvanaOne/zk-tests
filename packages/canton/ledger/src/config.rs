use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ledger_host: String,
    pub ledger_port: u16,
    pub validator_port: u16,
    pub jwt_secret: String,
    pub jwt_audience: String,
    pub jwt_user: String,
    pub party_id: String,
    pub use_tls: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ledger_host: "localhost".to_string(),
            ledger_port: 3901, // app-provider by default
            validator_port: 3903,
            jwt_secret: "unsafe".to_string(),
            jwt_audience: "https://canton.network.global".to_string(),
            jwt_user: "ledger-api-user".to_string(),
            party_id: "app_provider_localnet-localparty-1::122047631b9f7d279838384bfa3bfef3d1d8e35808707e1acc0f02355077aaab9eb7".to_string(),
            use_tls: false,
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self> {
        // Load .env file if it exists
        if Path::new(".env").exists() {
            dotenv::dotenv().ok();
        }

        let mut config = Config::default();

        if let Ok(host) = env::var("LEDGER_HOST") {
            config.ledger_host = host;
        }
        if let Ok(port) = env::var("LEDGER_PORT") {
            config.ledger_port = port.parse()?;
        }
        if let Ok(port) = env::var("VALIDATOR_PORT") {
            config.validator_port = port.parse()?;
        }
        if let Ok(secret) = env::var("JWT_SECRET") {
            config.jwt_secret = secret;
        }
        if let Ok(audience) = env::var("JWT_AUDIENCE") {
            config.jwt_audience = audience;
        }
        if let Ok(user) = env::var("JWT_USER") {
            config.jwt_user = user;
        }
        if let Ok(party) = env::var("PARTY_ID") {
            config.party_id = party;
        }
        if let Ok(tls) = env::var("USE_TLS") {
            config.use_tls = tls.parse()?;
        }

        Ok(config)
    }

    pub fn ledger_endpoint(&self) -> String {
        format!("{}:{}", self.ledger_host, self.ledger_port)
    }

    pub fn validator_endpoint(&self) -> String {
        // Check if we're running inside a Docker container
        if Path::new("/.dockerenv").exists() || Path::new("/run/.containerenv").exists() {
            // Inside container, try to use host.docker.internal or the Docker bridge
            // For devnet, the validator API is exposed via nginx on the host
            if self.validator_port == 5003 {
                // Use the host's nginx proxy which forwards to the validator API
                // The host is typically accessible at 172.17.0.1 (Docker bridge) or host.docker.internal
                if let Ok(host) = env::var("VALIDATOR_HOST") {
                    format!("http://{}", host)
                } else {
                    // Try common Docker host addresses
                    // First try host.docker.internal (works on Docker Desktop)
                    // Otherwise use Docker bridge gateway
                    "http://172.17.0.1".to_string()
                }
            } else {
                format!("http://{}:{}", self.ledger_host, self.validator_port)
            }
        } else if self.validator_port == 80 {
            // Using nginx proxy
            format!("http://{}", self.ledger_host)
        } else {
            format!("http://{}:{}", self.ledger_host, self.validator_port)
        }
    }
}