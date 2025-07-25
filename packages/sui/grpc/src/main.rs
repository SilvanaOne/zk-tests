mod checkpoint;
mod constants;
mod events_grpc;
mod events_rest;
mod proto;

const NUM_EVENTS: u32 = 1000;
const NUM_CHECKPOINTS: u32 = 50000;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize rustls crypto provider to fix TLS connections
    let _ = rustls::crypto::ring::default_provider().install_default();

    println!("🎯 Sui gRPC Client - Checkpoint Subscription Demo");
    println!("================================================");

    // Use gRPC to subscribe to checkpoints
    checkpoint::subscribe_to_checkpoints().await?;

    // Use gRPC and REST API to query events in parallel
    let (grpc_result, rest_result) = tokio::join!(
        events_grpc::query_events_via_grpc(NUM_EVENTS, NUM_CHECKPOINTS),
        events_rest::query_events_via_rest(NUM_EVENTS)
    );

    let (grpc_fresh_events, grpc_average_delay_ms, grpc_response_size) = grpc_result?;
    let (rest_fresh_events, rest_average_delay_ms, rest_response_size) = rest_result?;

    // Print summary of results
    println!("\n🎯 Final Results Comparison:");
    println!("============================================");
    println!("📊 gRPC Results:");
    println!("   • Fresh events found: {}", grpc_fresh_events);
    println!("   • Average delay: {:.2}ms", grpc_average_delay_ms);
    println!(
        "   • Total response size: {} bytes ({:.2} KB)",
        grpc_response_size,
        grpc_response_size as f64 / 1024.0
    );
    println!("📊 REST Results:");
    println!("   • Fresh events found: {}", rest_fresh_events);
    println!("   • Average delay: {:.2}ms", rest_average_delay_ms);
    println!(
        "   • Total response size: {} bytes ({:.2} KB)",
        rest_response_size,
        rest_response_size as f64 / 1024.0
    );

    Ok(())
}
