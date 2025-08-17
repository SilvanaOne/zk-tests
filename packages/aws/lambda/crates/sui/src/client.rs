use anyhow::Result;
use std::sync::OnceLock;
use sui_rpc::Client as GrpcClient;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Devnet,
    Testnet,
    Mainnet,
}

impl Network {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "devnet" => Ok(Network::Devnet),
            "testnet" => Ok(Network::Testnet),
            "mainnet" => Ok(Network::Mainnet),
            _ => Err(anyhow::anyhow!("Unknown network: {}. Please use 'devnet', 'testnet', or 'mainnet'.", s))
        }
    }

    pub fn rpc_url(&self) -> &'static str {
        match self {
            Network::Devnet => DEVNET_RPC_URL,
            Network::Testnet => TESTNET_RPC_URL,
            Network::Mainnet => MAINNET_RPC_URL,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Network::Devnet => "devnet",
            Network::Testnet => "testnet",
            Network::Mainnet => "mainnet",
        }
    }
}

static DEVNET_CLIENT: OnceLock<GrpcClient> = OnceLock::new();
static TESTNET_CLIENT: OnceLock<GrpcClient> = OnceLock::new();
static MAINNET_CLIENT: OnceLock<GrpcClient> = OnceLock::new();

const DEVNET_RPC_URL: &str = "https://fullnode.devnet.sui.io:443";
const TESTNET_RPC_URL: &str = "https://fullnode.testnet.sui.io:443";
const MAINNET_RPC_URL: &str = "https://fullnode.mainnet.sui.io:443";

/// Get a cached GRPC client for the given network (cloned for mutable use)
pub fn get_client(network: Network) -> GrpcClient {
    let start = std::time::Instant::now();
    let client = match network {
        Network::Devnet => {
            DEVNET_CLIENT.get_or_init(|| {
                GrpcClient::new(DEVNET_RPC_URL.to_string()).expect("Failed to create devnet client")
            })
        }
        Network::Testnet => {
            TESTNET_CLIENT.get_or_init(|| {
                GrpcClient::new(TESTNET_RPC_URL.to_string()).expect("Failed to create testnet client")
            })
        }
        Network::Mainnet => {
            MAINNET_CLIENT.get_or_init(|| {
                GrpcClient::new(MAINNET_RPC_URL.to_string()).expect("Failed to create mainnet client")
            })
        }
    };
    let result = client.clone();
    let creation_time = start.elapsed();
    info!("Client creation for {} took: {:?} ns", network.as_str(), creation_time.as_nanos());
    result
}

/// Get a cached GRPC client for the given network string (cloned for mutable use)
pub fn get_client_by_str(network: &str) -> Result<GrpcClient> {
    let network = Network::from_str(network)?;
    Ok(get_client(network))
}

