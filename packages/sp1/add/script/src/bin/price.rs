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

/// The ELF for the aggregation program
pub const AGGREGATE_ELF: &[u8] = include_elf!("aggregate-program");

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

    /// Number of price points per proof
    #[arg(long, default_value = "10")]
    operations: u32,

    /// Number of proofs to generate and aggregate (1 = no aggregation)
    #[arg(long, default_value = "1")]
    proofs: u32,

    /// Interval in seconds between price fetches
    #[arg(short, long, default_value = "5")]
    interval: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup the logger
    sp1_sdk::utils::setup_logger();
    dotenv::dotenv().ok();

    // Parse the command line arguments
    let args = Args::parse();

    if args.execute == args.prove {
        eprintln!("Error: You must specify either --execute or --prove");
        std::process::exit(1);
    }

    // Calculate total number of prices
    let total_prices = args.operations * args.proofs;

    // Convert token to Binance trading pair format (e.g., BTC -> BTCUSDT)
    let symbol = format!("{}USDT", args.token.to_uppercase());
    println!("=== Fetching Price Proof Data ===");
    println!("Trading pair: {}", symbol);
    println!("Number of prices: {}", total_prices);
    if args.proofs > 1 {
        println!("Proof strategy: {} proofs × {} prices each", args.proofs, args.operations);
    }
    if total_prices > 1 {
        println!("Interval: {} seconds", args.interval);
        if total_prices > 10 {
            println!("⚡ Optimization: Will fetch 10 unique prices and reuse them");
        }
        println!();
    }

    // Fetch price proof data using witness library
    let proof_data_list = if total_prices > 1 {
        fetch_multiple_prices(&symbol, total_prices, args.interval)
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
        println!("Price {}/{}:", i + 1, total_prices);
        println!("  Symbol:    {}", proof_data.price.symbol);
        println!("  Price:     ${}", proof_data.price.price);
        println!("  Timestamp: {}", proof_data.price.timestamp_fetched);
        println!("  Certificates: {}", proof_data.certificates.certificates_der.len());
        println!("  Checkpoint Sequence: {}", proof_data.checkpoint.sequence_number);
    }

    if args.execute {
        // Execute mode: Process all prices and execute zkVM
        // Initialize price manager
        let mut price_manager = PriceManager::new(16); // Height 16 supports 2^16 entries
        let initial_root = price_manager.get_root();

        // Setup the inputs for zkVM
        let mut stdin = SP1Stdin::new();

        // Write initial root
        let initial_root_bytes: [u8; 32] = initial_root.as_bytes().try_into().unwrap();
        stdin.write(&initial_root_bytes);

        // Write number of prices
        stdin.write(&total_prices);

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
                i + 1, total_prices, proof_data.price.price, proof_data.price.timestamp_fetched);
        }

        let final_root = price_manager.get_root();
        println!("\nInitial root: 0x{}", hex::encode(initial_root.as_bytes()));
        println!("Final root:   0x{}", hex::encode(final_root.as_bytes()));
        // Execute the program
        println!("\n=== Executing zkVM Program ===");
        let client = ProverClient::from_env();
        let (output, report) = client.execute(PRICE_ELF, &stdin).run()
            .map_err(|e| {
                eprintln!("\n❌ ERROR: zkVM execution failed!");
                eprintln!("Details: {}", e);
                eprintln!("\nDebug information:");
                eprintln!("  - Number of prices: {}", total_prices);
                eprintln!("  - Token: {}", args.token);
                e
            })?;
        println!("✓ Program executed successfully");

        // Read the output
        let output_bytes = output.as_slice();

        // Decode using the PublicValuesStruct (old_root, new_root)
        let decoded = PublicValuesStruct::abi_decode(output_bytes)
            .map_err(|e| {
                eprintln!("\n❌ ERROR: Failed to decode public values from execution output!");
                eprintln!("Details: {}", e);
                eprintln!("Output bytes length: {}", output_bytes.len());
                e
            })?;

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
        println!("Starting proof generation...");
        println!("  ELF size: {} bytes", PRICE_ELF.len());
        println!("  Input data prepared for {} prices", total_prices);
        println!("  Proof system: {:?}", args.system);

        if args.proofs == 1 {
            // Single proof mode: Process all prices and generate one proof
            // Initialize price manager
            let mut price_manager = PriceManager::new(16);
            let initial_root = price_manager.get_root();

            // Setup the inputs for zkVM
            let mut stdin = SP1Stdin::new();

            // Write initial root
            let initial_root_bytes: [u8; 32] = initial_root.as_bytes().try_into().unwrap();
            stdin.write(&initial_root_bytes);

            // Write number of prices
            stdin.write(&total_prices);

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
                    i + 1, total_prices, proof_data.price.price, proof_data.price.timestamp_fetched);
            }

            let final_root = price_manager.get_root();
            println!("\nInitial root: 0x{}", hex::encode(initial_root.as_bytes()));
            println!("Final root:   0x{}", hex::encode(final_root.as_bytes()));

            let setup_start = std::time::Instant::now();
            let client = ProverClient::builder().cpu().build();
            let (_pk, vk) = client.setup(PRICE_ELF);

            let proof = match add_script::proof::generate_single_proof(
                PRICE_ELF,
                &stdin,
                final_proof_type
            ) {
                Ok(proof) => {
                    println!("✓ Proof generation completed successfully");
                    proof
                }
                Err(e) => {
                    eprintln!("\n❌ ERROR: Proof generation failed!");
                    eprintln!("Error details: {}", e);
                    eprintln!("\nDebug information:");
                    eprintln!("  - Number of prices: {}", total_prices);
                    eprintln!("  - Proof system: {:?}", args.system);
                    eprintln!("  - Token: {}", args.token);

                    // Print the error chain
                    let mut source = e.source();
                    let mut level = 1;
                    while let Some(err) = source {
                        eprintln!("\nCaused by (level {}):", level);
                        eprintln!("  {}", err);
                        source = err.source();
                        level += 1;
                    }

                    return Err(e);
                }
            };

            let prove_duration = setup_start.elapsed();

            // Decode output first
            let output_bytes = proof.public_values.as_slice();
            let decoded = PublicValuesStruct::abi_decode(output_bytes)
                .map_err(|e| {
                    eprintln!("\n❌ ERROR: Failed to decode proof public values!");
                    eprintln!("Details: {}", e);
                    eprintln!("Output bytes: {} bytes", output_bytes.len());
                    e
                })?;

            // Print proof statistics
            print_price_proof_statistics(&proof, &decoded);

            // Verify the proof
            let verify_duration = add_script::proof::verify_proof(&proof, &vk)
                .map_err(|e| {
                    eprintln!("\n❌ ERROR: Proof verification failed!");
                    eprintln!("Details: {}", e);
                    eprintln!("The proof was generated but is invalid.");
                    e
                })?;

            println!("\n=== Performance Summary ===");
            println!("Total time:      {:.2}s", prove_duration.as_secs_f64());
            println!("Verification:    {:.2}s", verify_duration.as_secs_f64());

            // Save the core proof to JSON
            save_price_core_proof(&proof, &vk, &decoded)
                .map_err(|e| {
                    eprintln!("\n⚠️  WARNING: Failed to save proof to file!");
                    eprintln!("Details: {}", e);
                    eprintln!("The proof was generated successfully but couldn't be saved.");
                    e
                })?;
        } else {
            // Multiple proofs with aggregation
            println!("\n⚡ Aggregation mode: Generating {} proofs × {} prices each", args.proofs, args.operations);

            let aggregation_start = std::time::Instant::now();

            // Setup prover clients
            println!("Setting up proving keys...");
            let setup_start = std::time::Instant::now();
            let price_client = ProverClient::builder().cpu().build();
            let aggregate_client = ProverClient::builder().cpu().build();
            let (price_pk, price_vk) = price_client.setup(PRICE_ELF);
            let (aggregate_pk, aggregate_vk) = aggregate_client.setup(AGGREGATE_ELF);
            let setup_duration = setup_start.elapsed();
            println!("Setup completed in {:.2}s", setup_duration.as_secs_f64());

            // Generate individual proofs for each chunk
            println!("\nGenerating {} individual compressed proofs...", args.proofs);
            let prove_start = std::time::Instant::now();
            let mut proofs = Vec::new();

            // Create a single price manager that persists across all chunks
            // This ensures each chunk's old_root == previous chunk's new_root
            let mut chain_price_manager = PriceManager::new(16);

            for proof_idx in 0..args.proofs {
                println!("\n--- Proof {}/{} ---", proof_idx + 1, args.proofs);
                let chunk_start = std::time::Instant::now();

                let start_idx = (proof_idx * args.operations) as usize;
                let end_idx = ((proof_idx + 1) * args.operations) as usize;
                let chunk = &proof_data_list[start_idx..end_idx];

                // Use the current root as initial root for this chunk
                let chunk_initial_root = chain_price_manager.get_root();

                // Setup stdin for this chunk
                let mut chunk_stdin = SP1Stdin::new();
                let chunk_initial_root_bytes: [u8; 32] = chunk_initial_root.as_bytes().try_into().unwrap();
                chunk_stdin.write(&chunk_initial_root_bytes);
                chunk_stdin.write(&args.operations);

                // Process prices in this chunk
                println!("  Processing {} prices...", args.operations);
                for (i, proof_data) in chunk.iter().enumerate() {
                    let proof_bytes = borsh::to_vec(&proof_data).expect("Failed to serialize proof data");
                    chunk_stdin.write(&proof_bytes);

                    let witness = chain_price_manager
                        .process_price(proof_data.price.timestamp_fetched, &proof_data.price.price)
                        .expect("Failed to process price");

                    let witness_bytes = borsh::to_vec(&witness).expect("Failed to serialize witness");
                    chunk_stdin.write(&witness_bytes);

                    if (i + 1) % 10 == 0 || i == chunk.len() - 1 {
                        println!("    Processed {}/{} prices", i + 1, chunk.len());
                    }
                }

                let chunk_final_root = chain_price_manager.get_root();
                println!("  Old root: 0x{}", hex::encode(chunk_initial_root.as_bytes()));
                println!("  New root: 0x{}", hex::encode(chunk_final_root.as_bytes()));

                // Generate compressed proof for this chunk
                println!("  Generating compressed proof...");
                let proof_gen_start = std::time::Instant::now();
                let chunk_proof = price_client
                    .prove(&price_pk, &chunk_stdin)
                    .compressed()
                    .run()
                    .map_err(|e| {
                        eprintln!("\n❌ ERROR: Failed to generate proof {}/{}", proof_idx + 1, args.proofs);
                        eprintln!("Error: {}", e);
                        e
                    })?;
                let proof_gen_duration = proof_gen_start.elapsed();

                // Verify chunk output
                let decoded = PublicValuesStruct::abi_decode(chunk_proof.public_values.as_slice())
                    .expect("Failed to decode public values");

                let old_bytes: [u8; 32] = chunk_initial_root.as_bytes().try_into().unwrap();
                let expected_old = alloy_sol_types::private::U256::from_be_bytes(old_bytes);
                let new_bytes: [u8; 32] = chunk_final_root.as_bytes().try_into().unwrap();
                let expected_new = alloy_sol_types::private::U256::from_be_bytes(new_bytes);

                assert_eq!(decoded.old_root, expected_old, "Old root mismatch in proof {}", proof_idx);
                assert_eq!(decoded.new_root, expected_new, "New root mismatch in proof {}", proof_idx);

                let chunk_total_duration = chunk_start.elapsed();
                println!("  ✓ Proof {}/{} completed in {:.2}s (proving: {:.2}s)",
                    proof_idx + 1, args.proofs,
                    chunk_total_duration.as_secs_f64(),
                    proof_gen_duration.as_secs_f64());
                proofs.push((chunk_proof, price_vk.clone()));
            }

            let individual_prove_duration = prove_start.elapsed();
            println!("\n✓ Generated {} individual proofs in {:.2}s", args.proofs, individual_prove_duration.as_secs_f64());

            // Aggregate the proofs
            println!("\nAggregating {} proofs...", args.proofs);
            let aggregate_start = std::time::Instant::now();

            let mut aggregate_stdin = SP1Stdin::new();

            // Write verification keys
            let vkeys: Vec<[u32; 8]> = proofs.iter().map(|(_, vk)| vk.hash_u32()).collect();
            aggregate_stdin.write::<Vec<[u32; 8]>>(&vkeys);

            // Write public values
            let public_values: Vec<Vec<u8>> = proofs
                .iter()
                .map(|(proof, _)| proof.public_values.to_vec())
                .collect();
            aggregate_stdin.write::<Vec<Vec<u8>>>(&public_values);

            // Write the proofs for verification
            for (proof, vk) in &proofs {
                let sp1_sdk::SP1Proof::Compressed(compressed_proof) = &proof.proof else {
                    panic!("Expected compressed proof for aggregation")
                };
                aggregate_stdin.write_proof(*compressed_proof.clone(), vk.vk.clone());
            }

            // Generate final aggregated proof with selected proof system
            println!("Generating final {:?} proof from aggregation...", args.system);
            let final_proof = match final_proof_type {
                add_script::proof::FinalProofType::Core => aggregate_client
                    .prove(&aggregate_pk, &aggregate_stdin)
                    .compressed()
                    .run()?,
                add_script::proof::FinalProofType::Groth16 => aggregate_client
                    .prove(&aggregate_pk, &aggregate_stdin)
                    .groth16()
                    .run()?,
                add_script::proof::FinalProofType::Plonk => aggregate_client
                    .prove(&aggregate_pk, &aggregate_stdin)
                    .plonk()
                    .run()?,
            };

            let aggregate_duration = aggregate_start.elapsed();
            println!("✓ Aggregation completed in {:.2}s", aggregate_duration.as_secs_f64());

            // Verify the aggregated proof
            println!("\nVerifying aggregated proof...");
            let verify_start = std::time::Instant::now();
            aggregate_client.verify(&final_proof, &aggregate_vk)?;
            let verify_duration = verify_start.elapsed();
            println!("✅ Aggregated proof verified in {:.2}s", verify_duration.as_secs_f64());

            // Decode and verify aggregated values
            let aggregated_values = PublicValuesStruct::abi_decode(final_proof.public_values.as_slice())
                .expect("Failed to decode aggregated public values");

            let first_proof_values = PublicValuesStruct::abi_decode(proofs[0].0.public_values.as_slice())
                .expect("Failed to decode first proof");
            let last_proof_values = PublicValuesStruct::abi_decode(proofs[proofs.len() - 1].0.public_values.as_slice())
                .expect("Failed to decode last proof");

            assert_eq!(
                aggregated_values.old_root, first_proof_values.old_root,
                "Aggregated old_root mismatch"
            );
            assert_eq!(
                aggregated_values.new_root, last_proof_values.new_root,
                "Aggregated new_root mismatch"
            );

            println!("\n=== Aggregation Results ===");
            println!("Total proofs aggregated: {}", args.proofs);
            println!("Total prices proven: {}", total_prices);
            println!("Aggregated old_root: 0x{}", hex::encode(aggregated_values.old_root.to_be_bytes::<32>()));
            println!("Aggregated new_root: 0x{}", hex::encode(aggregated_values.new_root.to_be_bytes::<32>()));

            let total_duration = aggregation_start.elapsed();
            println!("\n=== Performance Summary ===");
            println!("Setup time:         {:.2}s", setup_duration.as_secs_f64());
            println!("Individual proofs:  {:.2}s", individual_prove_duration.as_secs_f64());
            println!("Aggregation:        {:.2}s", aggregate_duration.as_secs_f64());
            println!("Verification:       {:.2}s", verify_duration.as_secs_f64());
            println!("Total time:         {:.2}s", total_duration.as_secs_f64());

            // Print proof statistics
            print_price_proof_statistics(&final_proof, &aggregated_values);

            // Save the aggregated proof
            save_price_core_proof(&final_proof, &aggregate_vk, &aggregated_values)
                .map_err(|e| {
                    eprintln!("\n⚠️  WARNING: Failed to save proof to file!");
                    eprintln!("Details: {}", e);
                    e
                })?;
        }
    }

    Ok(())
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
