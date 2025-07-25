use chrono::{DateTime, Utc};
use prost_types;
use std::error::Error;
use tonic::Request;
use tonic::transport::{Channel, ClientTlsConfig};

pub mod sui_rpc {
    tonic::include_proto!("sui.rpc.v2beta2");
}

use sui_rpc::{
    GetCheckpointRequest, SubscribeCheckpointsRequest, ledger_service_client::LedgerServiceClient,
    subscription_service_client::SubscriptionServiceClient,
};

pub async fn subscribe_to_checkpoints() -> Result<(), Box<dyn Error>> {
    println!("🚀 Starting Sui gRPC client to subscribe to checkpoints");

    // Try to connect to Sui - first with TLS, then plaintext
    let channel = match create_sui_channel_tls().await {
        Ok(channel) => {
            println!("✅ Successfully connected to Sui gRPC with TLS!");
            channel
        }
        Err(tls_error) => {
            println!("⚠️  TLS connection failed: {}", tls_error);
            println!("🔄 Trying plaintext connection...");

            match create_sui_channel_plaintext().await {
                Ok(channel) => {
                    println!("✅ Successfully connected to Sui gRPC with plaintext!");
                    channel
                }
                Err(plaintext_error) => {
                    println!("❌ Both TLS and plaintext connections failed:");
                    println!("   TLS error: {}", tls_error);
                    println!("   Plaintext error: {}", plaintext_error);
                    println!("💡 Check network connectivity and endpoint availability");
                    println!("📝 gRPC client is properly generated and ready for use");
                    return Ok(());
                }
            }
        }
    };

    // Create subscription client for streaming checkpoints
    let mut subscription_client = SubscriptionServiceClient::new(channel.clone());

    // Create ledger client for fetching full checkpoint data
    let mut ledger_client = LedgerServiceClient::new(channel);

    // Subscribe to checkpoint stream
    let request = Request::new(SubscribeCheckpointsRequest {
        read_mask: None, // Request all available data
    });

    println!("📡 Subscribing to checkpoint stream...");
    let mut stream = subscription_client
        .subscribe_checkpoints(request)
        .await?
        .into_inner();

    let mut checkpoint_count = 0;
    const MAX_CHECKPOINTS: u32 = 5; // Limit for demo

    // Process each checkpoint
    while let Some(checkpoint_response) = stream.message().await? {
        checkpoint_count += 1;

        if let Some(cursor) = checkpoint_response.cursor {
            println!("📍 Checkpoint cursor: {}", cursor);

            // Fetch full checkpoint data using the cursor
            let checkpoint_request = Request::new(GetCheckpointRequest {
                checkpoint_id: Some(
                    sui_rpc::get_checkpoint_request::CheckpointId::SequenceNumber(cursor),
                ),
                read_mask: Some(prost_types::FieldMask {
                    paths: vec![
                        "sequence_number".to_string(),
                        "digest".to_string(),
                        "summary".to_string(),
                    ],
                }),
            });

            match ledger_client.get_checkpoint(checkpoint_request).await {
                Ok(response) => {
                    if let Some(checkpoint) = response.into_inner().checkpoint {
                        display_checkpoint(&checkpoint, checkpoint_count);
                    } else {
                        println!("⚠️  No checkpoint data returned from ledger service");
                    }
                }
                Err(e) => {
                    println!("❌ Failed to fetch checkpoint {}: {}", cursor, e);
                }
            }
        } else {
            println!("⚠️  Received response with no cursor");
        }

        // Stop after processing a few checkpoints for demo
        if checkpoint_count >= MAX_CHECKPOINTS {
            println!(
                "\n✅ Processed {} checkpoints, stopping demo",
                MAX_CHECKPOINTS
            );
            break;
        }
    }

    Ok(())
}

async fn create_sui_channel_tls() -> Result<Channel, Box<dyn Error>> {
    // Sui gRPC endpoint
    let endpoint = "https://148.251.75.59:9000";

    // Create TLS configuration with webpki roots
    let tls = ClientTlsConfig::new().domain_name("fullnode.mainnet.sui.io");

    let channel = Channel::from_static(endpoint)
        .tls_config(tls)?
        .connect()
        .await?;

    println!("✅ Connected to Sui gRPC with TLS at {}", endpoint);
    Ok(channel)
}

async fn create_sui_channel_plaintext() -> Result<Channel, Box<dyn Error>> {
    // Sui gRPC endpoint (plaintext)
    let endpoint = "http://148.251.75.59:9000";

    let channel = Channel::from_static(endpoint).connect().await?;

    println!("✅ Connected to Sui gRPC with plaintext at {}", endpoint);
    Ok(channel)
}

fn display_checkpoint(checkpoint: &sui_rpc::Checkpoint, checkpoint_number: u32) {
    println!("\n🏗️  Checkpoint #{} received!", checkpoint_number);
    println!("┌─────────────────────────────────────────────────────────────────");

    if let Some(sequence_number) = checkpoint.sequence_number {
        println!("│ Sequence Number: {}", sequence_number);
    }

    if let Some(digest) = &checkpoint.digest {
        println!("│ Digest:          {}", digest);
    }

    if let Some(summary) = &checkpoint.summary {
        if let Some(epoch) = summary.epoch {
            println!("│ Epoch:           {}", epoch);
        }

        if let Some(previous_digest) = &summary.previous_digest {
            println!("│ Previous:        {}", previous_digest);
        }

        // Display timestamp
        if let Some(timestamp) = &summary.timestamp {
            // Convert Unix timestamp to human-readable format
            if let Some(datetime) =
                DateTime::from_timestamp(timestamp.seconds, timestamp.nanos as u32)
            {
                // Calculate delay from checkpoint creation to now
                let now = Utc::now();
                let delay = now - datetime;
                let delay_ms = delay.num_milliseconds();

                println!(
                    "│ Timestamp:       {} (delay: {}ms)",
                    datetime.format("%Y-%m-%d %H:%M:%S%.3f UTC"),
                    delay_ms
                );
            } else {
                println!(
                    "│ Timestamp:       {}s + {}ns (invalid)",
                    timestamp.seconds, timestamp.nanos
                );
            }
        }
    }

    // Check if there are any transactions in this checkpoint
    let transaction_count = checkpoint.transactions.len();
    if transaction_count > 0 {
        println!(
            "│ Transactions:    {} in this checkpoint",
            transaction_count
        );

        // Count events from our target package
        let target_package = "0x2c8d603bc51326b8c13cef9dd07031a408a48dddb541963357661df5d3204809";
        let mut event_count = 0;

        for tx in &checkpoint.transactions {
            if let Some(events) = &tx.events {
                for event in &events.events {
                    if let Some(package_id) = &event.package_id {
                        if package_id == target_package {
                            event_count += 1;
                        }
                    }
                }
            }
        }

        if event_count > 0 {
            println!(
                "│ 🎯 Target Events: {} events from our package!",
                event_count
            );
        }
    }

    // Display link to Suiscan
    if let Some(sequence_number) = checkpoint.sequence_number {
        println!(
            "│ 🌐 View on Suiscan: https://suiscan.xyz/testnet/checkpoint/{}",
            sequence_number
        );
    }

    println!("└─────────────────────────────────────────────────────────────────");
}
