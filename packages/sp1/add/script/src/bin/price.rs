//! Price verification zkVM execution script
//!
//! This script fetches real price data using the witness library and executes
//! the price verification zkVM program to verify all cryptographic proofs.
//!
//! You can run this script using the following command:
//! ```shell
//! RUST_LOG=info cargo run --release --bin price -- --execute --token BTC
//! ```

use add_script::proof::FinalProofType;
use alloy_sol_types::{sol, SolType};
use borsh;
use clap::{Parser, ValueEnum};
use indexed_merkle_map::{Hash, IndexedMerkleMap, InsertWitness, Field};
use serde::{Deserialize, Serialize};
use sp1_sdk::{HashableKey, Prover, ProverClient, SP1ProofWithPublicValues, SP1Stdin, SP1VerifyingKey, include_elf};
use std::path::PathBuf;

/// The ELF (executable and linkable format) file for the price verification program
pub const PRICE_ELF: &[u8] = include_elf!("price-program");

// Define the same PublicValuesStruct as in the price program
sol! {
    /// The public values encoded as a struct that can be easily deserialized inside Solidity.
    struct PublicValuesStruct {
        uint256 old_root;
        uint256 new_root;
    }
}

/// Price proof fixture for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceProofFixture {
    pub old_root: String,
    pub new_root: String,
    pub vkey: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vkey_bytes: Option<String>,
    pub public_values: String,
    pub proof: String,
}

/// Price Manager using IndexedMerkleMap
struct PriceManager {
    map: IndexedMerkleMap,
}

impl PriceManager {
    fn new(height: usize) -> Self {
        Self {
            map: IndexedMerkleMap::new(height),
        }
    }

    fn process_price(
        &mut self,
        timestamp: u64,
        price: &str,
    ) -> Result<InsertWitness, Box<dyn std::error::Error>> {
        let key = Field::from_bytes(price_lib::conversion::timestamp_to_bytes(timestamp));
        let value = Field::from_bytes(price_lib::conversion::price_to_bytes(price)?);

        // Insert only - map is append-only
        let witness = self
            .map
            .insert_and_generate_witness(key, value, true)?
            .ok_or("Failed to generate insert witness")?;
        Ok(witness)
    }

    fn get_root(&self) -> Hash {
        self.map.root()
    }
}

/// Fetch multiple price proofs with specified interval
///
/// Optimization: For counts > 10, fetches only 10 unique price points
/// and reuses them cyclically to reach the desired count. This drastically
/// reduces API calls and fetch time while maintaining test data diversity.
async fn fetch_multiple_prices(
    symbol: &str,
    count: u32,
    interval_secs: u64,
) -> Result<Vec<price_lib::PriceProofData>, Box<dyn std::error::Error>> {
    use tokio::time::{sleep, Duration};

    // Optimization: For large counts, fetch max 10 unique prices and reuse
    let fetch_count = std::cmp::min(count, 10);
    let mut unique_proofs = Vec::new();

    println!("Fetching {} unique price points...", fetch_count);
    for i in 0..fetch_count {
        println!("Fetching price {}/{}...", i + 1, fetch_count);
        let proof_data = witness::fetch_price_proof_data(symbol).await?;
        unique_proofs.push(proof_data);

        if i < fetch_count - 1 {
            println!("Waiting {} seconds before next fetch...\n", interval_secs);
            sleep(Duration::from_secs(interval_secs)).await;
        }
    }

    // If count > 10, reuse the fetched data cyclically with updated timestamps
    if count > fetch_count {
        println!("\nReusing {} price points to reach total count of {}...", fetch_count, count);
        let mut all_proofs = Vec::with_capacity(count as usize);

        for i in 0..count {
            // Add 10ms delay between items to ensure distinct timestamps
            if i > 0 {
                sleep(Duration::from_millis(10)).await;
            }

            let mut proof_data = unique_proofs[(i as usize) % (fetch_count as usize)].clone();

            // Update timestamp to current time to ensure uniqueness
            // This maintains temporal ordering while reusing the same price/certificate data
            proof_data.price.timestamp_fetched = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            all_proofs.push(proof_data);
        }
        Ok(all_proofs)
    } else {
        Ok(unique_proofs)
    }
}

/// Enum representing the available proof systems
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum ProofSystem {
    /// Compressed core proof (smallest, for verification aggregation)
    Compressed,
    /// Groth16 EVM-compatible proof (constant size ~300 bytes)
    Groth16,
    /// PLONK EVM-compatible proof
    Plonk,
}

/// The arguments for the command
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    execute: bool,

    #[arg(long)]
    prove: bool,

    /// Proof system to use (groth16, plonk, or compressed)
    #[arg(long, value_enum, default_value = "groth16")]
    system: ProofSystem,

    /// Token/trading pair to fetch (e.g., BTC, ETH, SOL)
    /// Will be converted to {TOKEN}USDT format for Binance API
    #[arg(short, long, default_value = "BTC")]
    token: String,

    /// Number of price points to fetch
    #[arg(short, long, default_value = "1")]
    count: u32,

    /// Interval in seconds between price fetches
    #[arg(short, long, default_value = "5")]
    interval: u64,
}

#[tokio::main]
async fn main() {
    // Setup the logger
    sp1_sdk::utils::setup_logger();
    dotenv::dotenv().ok();

    // Parse the command line arguments
    let args = Args::parse();

    if args.execute == args.prove {
        eprintln!("Error: You must specify either --execute or --prove");
        std::process::exit(1);
    }

    // Convert token to Binance trading pair format (e.g., BTC -> BTCUSDT)
    let symbol = format!("{}USDT", args.token.to_uppercase());
    println!("=== Fetching Price Proof Data ===");
    println!("Trading pair: {}", symbol);
    println!("Number of prices: {}", args.count);
    if args.count > 1 {
        println!("Interval: {} seconds", args.interval);
        if args.count > 10 {
            println!("⚡ Optimization: Will fetch 10 unique prices and reuse them");
        }
        println!();
    }

    // Fetch price proof data using witness library
    let proof_data_list = if args.count > 1 {
        fetch_multiple_prices(&symbol, args.count, args.interval)
            .await
            .expect("Failed to fetch price proof data")
    } else {
        vec![witness::fetch_price_proof_data(&symbol)
            .await
            .expect("Failed to fetch price proof data")]
    };

    // Print summary for all prices
    println!("\n=== Price Proof Data Summary ===");
    for (i, proof_data) in proof_data_list.iter().enumerate() {
        println!("Price {}/{}:", i + 1, args.count);
        println!("  Symbol:    {}", proof_data.price.symbol);
        println!("  Price:     ${}", proof_data.price.price);
        println!("  Timestamp: {}", proof_data.price.timestamp_fetched);
        println!("  Certificates: {}", proof_data.certificates.certificates_der.len());
        println!("  Checkpoint Sequence: {}", proof_data.checkpoint.sequence_number);
    }

    // Initialize price manager
    let mut price_manager = PriceManager::new(16); // Height 16 supports 2^16 entries
    let initial_root = price_manager.get_root();

    // Setup the inputs for zkVM
    let mut stdin = SP1Stdin::new();

    // Write initial root
    let initial_root_bytes: [u8; 32] = initial_root.as_bytes().try_into().unwrap();
    stdin.write(&initial_root_bytes);

    // Write number of prices
    stdin.write(&args.count);

    // Process each price and collect witnesses
    println!("\n=== Processing Prices in IndexedMerkleMap ===");
    for (i, proof_data) in proof_data_list.iter().enumerate() {
        // Serialize proof data using borsh
        let proof_bytes = borsh::to_vec(&proof_data).expect("Failed to serialize proof data");
        stdin.write(&proof_bytes);

        // Generate insert witness
        let witness = price_manager
            .process_price(proof_data.price.timestamp_fetched, &proof_data.price.price)
            .expect("Failed to process price");

        // Serialize witness
        let witness_bytes = borsh::to_vec(&witness).expect("Failed to serialize witness");
        stdin.write(&witness_bytes);

        println!("Processed price {}/{}: ${} at timestamp {}",
            i + 1, args.count, proof_data.price.price, proof_data.price.timestamp_fetched);
    }

    let final_root = price_manager.get_root();
    println!("\nInitial root: 0x{}", hex::encode(initial_root.as_bytes()));
    println!("Final root:   0x{}", hex::encode(final_root.as_bytes()));

    if args.execute {
        // Execute the program
        println!("\n=== Executing zkVM Program ===");
        let client = ProverClient::from_env();
        let (output, report) = client.execute(PRICE_ELF, &stdin).run().unwrap();
        println!("✓ Program executed successfully");

        // Read the output
        let output_bytes = output.as_slice();

        // Decode using the PublicValuesStruct (old_root, new_root)
        let decoded = PublicValuesStruct::abi_decode(output_bytes)
            .expect("Failed to decode output");

        println!("\n=== zkVM Output (Public Values) ===");
        println!("Old root: 0x{}", hex::encode(decoded.old_root.to_be_bytes::<32>()));
        println!("New root: 0x{}", hex::encode(decoded.new_root.to_be_bytes::<32>()));

        // Verify output matches computed roots
        println!("\n=== Verification ===");
        let initial_root_bytes: [u8; 32] = initial_root.as_bytes().try_into().unwrap();
        let expected_old = alloy_sol_types::private::U256::from_be_bytes(initial_root_bytes);
        let final_root_bytes: [u8; 32] = final_root.as_bytes().try_into().unwrap();
        let expected_new = alloy_sol_types::private::U256::from_be_bytes(final_root_bytes);
        println!("Old root matches: {}", decoded.old_root == expected_old);
        println!("New root matches: {}", decoded.new_root == expected_new);

        // Record the number of cycles executed
        println!("\n=== Execution Statistics ===");
        println!("Number of cycles:   {}", report.total_instruction_count());
        println!("Number of syscalls: {}", report.total_syscall_count());
        println!(
            "Total instructions: {}",
            report.total_instruction_count() + report.total_syscall_count()
        );

        // Print detailed execution report
        println!("\n=== Detailed Execution Report ===");
        println!("{report}");
    } else {
        // Generate proof using selected proof system
        let final_proof_type = match args.system {
            ProofSystem::Compressed => FinalProofType::Core,
            ProofSystem::Groth16 => FinalProofType::Groth16,
            ProofSystem::Plonk => FinalProofType::Plonk,
        };

        println!("\n=== Generating Proof ===");
        let setup_start = std::time::Instant::now();
        let client = ProverClient::builder().cpu().build();
        let (_pk, vk) = client.setup(PRICE_ELF);

        let proof = add_script::proof::generate_single_proof(
            PRICE_ELF,
            &stdin,
            final_proof_type
        ).expect("failed to generate proof");

        let prove_duration = setup_start.elapsed();

        // Decode output first
        let output_bytes = proof.public_values.as_slice();
        let decoded = PublicValuesStruct::abi_decode(output_bytes)
            .expect("Failed to decode output");

        // Print proof statistics
        print_price_proof_statistics(&proof, &decoded);

        // Verify the proof
        let verify_duration = add_script::proof::verify_proof(&proof, &vk)
            .expect("failed to verify proof");

        println!("\n=== Performance Summary ===");
        println!("Total time:      {:.2}s", prove_duration.as_secs_f64());
        println!("Verification:    {:.2}s", verify_duration.as_secs_f64());

        // Save the core proof to JSON
        save_price_core_proof(&proof, &vk, &decoded).expect("failed to save price core proof");
    }
}

/// Print price proof statistics
fn print_price_proof_statistics(
    proof: &SP1ProofWithPublicValues,
    decoded: &PublicValuesStruct,
) {
    println!("\n=== Proof Statistics ===");

    match &proof.proof {
        sp1_sdk::SP1Proof::Core(_) => println!("Proof type: Core"),
        sp1_sdk::SP1Proof::Compressed(_) => println!("Proof type: Compressed"),
        sp1_sdk::SP1Proof::Plonk(_) => println!("Proof type: PLONK"),
        sp1_sdk::SP1Proof::Groth16(_) => println!("Proof type: Groth16"),
    }

    println!(
        "Public values size: {} bytes",
        proof.public_values.as_slice().len()
    );

    println!("Old root: 0x{}", hex::encode(decoded.old_root.to_be_bytes::<32>()));
    println!("New root: 0x{}", hex::encode(decoded.new_root.to_be_bytes::<32>()));
}

/// Save price core proof to JSON file
fn save_price_core_proof(
    proof: &SP1ProofWithPublicValues,
    vk: &SP1VerifyingKey,
    decoded: &PublicValuesStruct,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Serialize the proof using bincode
    let proof_bytes = bincode::serialize(proof)?;

    // Serialize the verifying key
    let vkey_bytes = bincode::serialize(vk)?;

    // Get public values bytes
    let public_values_bytes = proof.public_values.as_slice();

    // Print proof size information
    println!("\n📊 Proof Size Information:");
    println!("   Serialized proof size: {} bytes ({:.2} KB)",
        proof_bytes.len(),
        proof_bytes.len() as f64 / 1024.0
    );
    println!("   Verifying key size: {} bytes ({:.2} KB)",
        vkey_bytes.len(),
        vkey_bytes.len() as f64 / 1024.0
    );
    println!("   Public values size: {} bytes", public_values_bytes.len());

    // Create the fixture
    let fixture = PriceProofFixture {
        old_root: format!("0x{}", hex::encode(decoded.old_root.to_be_bytes::<32>())),
        new_root: format!("0x{}", hex::encode(decoded.new_root.to_be_bytes::<32>())),
        vkey: vk.bytes32().to_string(),
        vkey_bytes: Some(format!("0x{}", hex::encode(vkey_bytes))),
        public_values: format!("0x{}", hex::encode(public_values_bytes)),
        proof: format!("0x{}", hex::encode(proof_bytes)),
    };

    // Create proofs directory if it doesn't exist
    let proof_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../proofs");
    std::fs::create_dir_all(&proof_dir)?;

    // Generate filename with timestamp
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let filename = format!("price-core-proof-{}.json", timestamp);
    let filepath = proof_dir.join(&filename);

    // Write to file
    let json_content = serde_json::to_string_pretty(&fixture)?;
    std::fs::write(&filepath, &json_content)?;

    // Print save confirmation
    println!("\n✅ Core proof saved to: proofs/{}", filename);
    println!("   File size: {} bytes ({:.2} KB)",
        json_content.len(),
        json_content.len() as f64 / 1024.0
    );
    println!("   Verification key: {}", fixture.vkey);

    Ok(filepath)
}
