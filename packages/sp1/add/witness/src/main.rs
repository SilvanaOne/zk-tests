mod binance;
mod price_proof;
mod sui;
mod tsa;

use anyhow::Result;
use chrono::{DateTime, NaiveDateTime, Utc};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "witness")]
#[command(about = "SP1 Witness CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Fetch price with full proof (price + certificates + time attestations)
    Proof,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Initialize tracing for logging
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Proof => {
            handle_proof_command().await?;
        }
    }

    Ok(())
}

async fn handle_proof_command() -> Result<()> {
    println!("=== Fetching Price Proof Data ===\n");

    // 1. Fetch all data
    let proof_data = price_proof::fetch_price_proof_data().await?;

    // 2. Verify everything
    println!("\n=== Verifying Proof Data ===\n");
    let verification = price_lib::verify_proof_data(&proof_data)?;

    // 3. Display results
    display_proof_results(&proof_data, &verification)?;

    Ok(())
}

fn display_proof_results(
    proof: &price_lib::PriceProofData,
    verification: &price_lib::VerificationResult,
) -> Result<()> {
    println!("\n=== PROOF DATA SUMMARY ===\n");

    // Price information
    println!("💰 Price Data:");
    println!("   Symbol:    {}", proof.price.symbol);
    println!("   Price:     ${}", proof.price.price);
    let price_dt = DateTime::<Utc>::from_timestamp_millis(proof.price.timestamp_fetched as i64)
        .unwrap_or_default();
    println!(
        "   Fetched:   {}",
        price_dt.format("%Y-%m-%d %H:%M:%S%.3f UTC")
    );

    // Certificate information
    println!("\n🔒 TLS Certificate Chain:");
    println!("   Certificates: {}", proof.certificates.certificates_der.len());
    println!(
        "   Leaf (Binance): {}",
        &proof.certificates.leaf_fingerprint[..16]
    );
    println!(
        "   Root (DigiCert): {}",
        &proof.certificates.root_fingerprint[..16]
    );

    // Checkpoint information
    println!("\n⏱️  Sui Checkpoint:");
    println!("   Sequence:  {}", proof.checkpoint.sequence_number);
    println!("   Epoch:     {}", proof.checkpoint.epoch);
    let checkpoint_dt =
        DateTime::<Utc>::from_timestamp_millis(proof.checkpoint.timestamp_ms as i64)
            .unwrap_or_default();
    println!(
        "   Timestamp: {}",
        checkpoint_dt.format("%Y-%m-%d %H:%M:%S%.3f UTC")
    );
    println!("   Digest:    {}...", &proof.checkpoint.digest[..16]);

    // TSA information
    println!("\n🕐 TSA Timestamp:");
    println!("   Time:      {}", proof.tsa_timestamp.time_string);
    println!("   Cert Verified: {}", if proof.tsa_timestamp.cert_verified { "✅ Yes" } else { "❌ No" });
    println!("   Certificates:  {}", proof.tsa_timestamp.cert_count);
    if let Some(subject) = &proof.tsa_timestamp.signer_cert_subject {
        println!("   Signer:        {}...", &subject[..subject.len().min(60)]);
    }
    if let Some(error) = &proof.tsa_timestamp.verification_error {
        println!("   Error:         {}", error);
    }

    // Parse TSA time for better display
    let tsa_time_str = proof.tsa_timestamp.time_string.replace("Z", "");
    if tsa_time_str.len() >= 14 {
        if let Ok(naive_dt) = NaiveDateTime::parse_from_str(&tsa_time_str, "%Y%m%d%H%M%S") {
            let tsa_datetime: DateTime<Utc> = DateTime::from_naive_utc_and_offset(naive_dt, Utc);
            println!(
                "   Parsed:    {}",
                tsa_datetime.format("%Y-%m-%d %H:%M:%S UTC")
            );

            // Calculate time differences
            let tsa_time_ms = tsa_datetime.timestamp_millis() as u64;
            let checkpoint_diff = if proof.checkpoint.timestamp_ms > tsa_time_ms {
                proof.checkpoint.timestamp_ms - tsa_time_ms
            } else {
                tsa_time_ms - proof.checkpoint.timestamp_ms
            };
            println!("   Diff from checkpoint: {:.3} seconds", checkpoint_diff as f64 / 1000.0);
        }
    }

    // Proof hash
    println!("\n🔐 Proof Data Hash:");
    println!("   {}", proof.data_hash);

    // Verification results
    println!("\n=== VERIFICATION RESULTS ===\n");

    for detail in &verification.details {
        println!("  {}", detail);
    }

    println!();
    if verification.all_verified {
        println!("✅ ALL VERIFICATIONS PASSED!");
        println!("\nThis proof demonstrates that:");
        println!("  • The BTC price ${} was fetched from authentic Binance API", proof.price.price);
        println!("  • The TLS certificate chain is valid and trusted");
        println!("  • The time is attested by both Sui checkpoint and TSA");
        println!("  • All timestamps are consistent and within acceptable bounds");
    } else {
        println!("❌ SOME VERIFICATIONS FAILED");
        println!("\nPlease review the details above.");
    }

    Ok(())
}
