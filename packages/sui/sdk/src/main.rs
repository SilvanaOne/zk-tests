mod constants;
mod events;

use anyhow::Result;
use rustls::crypto::ring::default_provider;
use sui_rpc::Client;
use sui_rpc::client::ResponseExt;
use sui_rpc::proto::sui::rpc::v2beta2::GetServiceInfoRequest;

const NUM_OF_EVENTS: usize = 10;

#[tokio::main]
async fn main() -> Result<()> {
    // Install default crypto provider for TLS
    default_provider()
        .install_default()
        .expect("Failed to install default crypto provider");

    // Initialize client for Sui testnet
    let mut client = Client::new("http://148.251.75.59:9000")?; // https://fullnode.testnet.sui.io:443

    // Get a ledger service client
    let mut ledger_client = client.ledger_client();

    // Example: Get service info
    let request = GetServiceInfoRequest {};
    let service_info = ledger_client.get_service_info(request).await?;
    println!("Chain: {:?}\n", service_info.chain().unwrap());

    // Stream checkpoints until we get at least NUM_OF_EVENTS target events
    events::stream_until_target_events(&mut client, NUM_OF_EVENTS).await?;

    Ok(())
}
