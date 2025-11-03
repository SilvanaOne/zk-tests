use alloy::network::{EthereumWallet, TransactionBuilder};
use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Use existing private key or generate a new one
    let signer = if let Ok(pk) = std::env::var("PRIVATE_KEY") {
        let pk_str = pk.trim_start_matches("0x");
        PrivateKeySigner::from_str(pk_str)?
    } else {
        PrivateKeySigner::random()
    };

    // Get the private key as bytes
    let private_key = signer.credential();

    // Get the Ethereum address
    let address = signer.address();

    // Print the private key in hex format
    println!("Private Key: 0x{}", hex::encode(private_key.to_bytes()));

    // Print the public address
    println!("Address: {}", address);

    // Get RPC URL and destination address from environment variables
    let rpc_url = std::env::var("ETHEREUM_RPC_URL")?;
    let destination_address = Address::from_str(&std::env::var("ADDRESS")?)?;

    // Create provider with wallet
    let wallet = EthereumWallet::from(signer);
    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .on_http(rpc_url.parse()?);

    // Get initial balance with retry
    let mut balance = loop {
        match provider.get_balance(address).await {
            Ok(b) => break b,
            Err(e) => {
                println!("Error getting balance: {}. Retrying in 10 seconds...", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            }
        }
    };
    let balance_eth = balance.to_string().parse::<f64>()? / 1e18;
    println!("Balance: {} ETH ({} wei)", balance_eth, balance);

    // Wait for positive balance
    while balance == U256::ZERO {
        println!("Waiting for funds... checking again in 10 seconds");
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        balance = loop {
            match provider.get_balance(address).await {
                Ok(b) => break b,
                Err(e) => {
                    println!("Error getting balance: {}. Retrying in 10 seconds...", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                }
            }
        };
        let balance_eth = balance.to_string().parse::<f64>()? / 1e18;
        println!("Balance: {} ETH ({} wei)", balance_eth, balance);
    }

    println!("Funds received! Preparing to transfer to {}", destination_address);

    // Get chain ID and gas price with retry
    let chain_id = loop {
        match provider.get_chain_id().await {
            Ok(id) => break id,
            Err(e) => {
                println!("Error getting chain ID: {}. Retrying in 5 seconds...", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    };
    let gas_price = loop {
        match provider.get_gas_price().await {
            Ok(price) => break price,
            Err(e) => {
                println!("Error getting gas price: {}. Retrying in 5 seconds...", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    };
    let gas_limit = 21000u64;
    let gas_cost = U256::from(gas_price) * U256::from(gas_limit);

    // Calculate amount to send (balance - gas cost)
    if balance <= gas_cost {
        return Err("Insufficient balance to cover gas fees".into());
    }
    let amount_to_send = balance - gas_cost;

    println!("Gas limit: {}", gas_limit);
    println!("Gas price: {} wei", gas_price);
    println!("Gas cost: {} wei", gas_cost);
    println!("Sending: {} wei", amount_to_send);

    // Create and send transaction with actual amount (with retry)
    let nonce = loop {
        match provider.get_transaction_count(address).await {
            Ok(n) => break n,
            Err(e) => {
                println!("Error getting nonce: {}. Retrying in 5 seconds...", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    };

    let tx = TransactionRequest::default()
        .with_to(destination_address)
        .with_value(amount_to_send)
        .with_from(address)
        .with_gas_limit(gas_limit)
        .with_gas_price(gas_price)
        .with_nonce(nonce)
        .with_chain_id(chain_id);

    loop {
        let pending_tx = match provider.send_transaction(tx.clone()).await {
            Ok(ptx) => ptx,
            Err(e) => {
                println!("Error sending transaction: {}. Retrying in 5 seconds...", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            }
        };

        let hash = *pending_tx.tx_hash();
        println!("Transaction sent: {}", hash);

        match pending_tx.get_receipt().await {
            Ok(r) => {
                println!("Transaction confirmed in block: {:?}", r.block_number);
                break;
            },
            Err(e) => {
                println!("Error getting receipt: {}. Retrying transaction in 5 seconds...", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    }

    // Wait 5 seconds and check remaining balance
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let final_balance = loop {
        match provider.get_balance(address).await {
            Ok(b) => break b,
            Err(e) => {
                println!("Error getting final balance: {}. Retrying in 5 seconds...", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    };
    let final_balance_eth = final_balance.to_string().parse::<f64>()? / 1e18;
    println!("Remaining balance: {} ETH ({} wei)", final_balance_eth, final_balance);

    Ok(())
}
