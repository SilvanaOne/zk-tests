//! An end-to-end example of using the SP1 SDK to generate a proof of a program that can be executed
//! or have a core proof generated.
//!
//! You can run this script using the following command:
//! ```shell
//! RUST_LOG=info cargo run --release -- --execute
//! ```
//! or
//! ```shell
//! RUST_LOG=info cargo run --release -- --prove
//! ```

use add_lib::PublicValuesStruct;
use alloy_sol_types::SolType;
use clap::Parser;
use sp1_sdk::{ProverClient, SP1Stdin, include_elf};
use std::time::Instant;

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const ADD_ELF: &[u8] = include_elf!("add-program");

/// The arguments for the command.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    execute: bool,

    #[arg(long)]
    prove: bool,

    #[arg(long, default_value = "10")]
    length: u32,
}

fn main() {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();
    dotenv::dotenv().ok();

    // Parse the command line arguments.
    let args = Args::parse();

    if args.execute == args.prove {
        eprintln!("Error: You must specify either --execute or --prove");
        std::process::exit(1);
    }

    // Setup the prover client.
    let client = ProverClient::from_env();

    // Setup the inputs.
    let mut stdin = SP1Stdin::new();
    let old_sum = 0u32;
    let values: Vec<u32> = (10..10 + args.length).collect();

    stdin.write(&old_sum);
    stdin.write(&args.length);
    for &value in &values {
        stdin.write(&value);
    }

    println!("old_sum: {old_sum}",);
    println!("length: {}", args.length);
    println!("values: {values:?}");

    if args.execute {
        // Execute the program
        let (output, report) = client.execute(ADD_ELF, &stdin).run().unwrap();
        println!("Program executed successfully.");

        // Read the output.
        let decoded = PublicValuesStruct::abi_decode(output.as_slice()).unwrap();
        let PublicValuesStruct { old_sum, new_sum } = decoded;
        println!("old_sum: {old_sum}");
        println!("new_sum: {new_sum}");

        let expected_new_sum = add_lib::add_many(&values, old_sum);
        assert_eq!(new_sum, expected_new_sum);
        println!("Values are correct!");

        // Record the number of cycles executed.
        println!("Number of cycles: {}", report.total_instruction_count());
        println!("Number of syscalls: {}", report.total_syscall_count());
        println!("Total instructions: {}", report.total_instruction_count() + report.total_syscall_count());
        println!("Touched memory addresses: {}", report.touched_memory_addresses);
        
        // Print detailed execution report
        println!("\n=== Detailed Execution Report ===");
        println!("{report}");
    } else {
        // Setup the program for proving.
        println!("Setting up proving keys...");
        let setup_start = Instant::now();
        let (pk, vk) = client.setup(ADD_ELF);
        let setup_duration = setup_start.elapsed();
        println!("Setup completed in {:.2}s", setup_duration.as_secs_f64());

        // Generate the proof
        println!("\nGenerating core proof...");
        let prove_start = Instant::now();
        let proof = client
            .prove(&pk, &stdin)
            .core()
            .run()
            .expect("failed to generate proof");
        let prove_duration = prove_start.elapsed();
        
        println!("Successfully generated core proof!");
        println!("Proving time: {:.2}s", prove_duration.as_secs_f64());
        
        // Print proof statistics
        println!("\n=== SP1 Core Proof Statistics ===");
        println!("Proof type: {}", proof.proof);
        println!("SP1 version: {}", proof.sp1_version);
        
        // Get proof size information using SP1's standard approach
        match &proof.proof {
            sp1_sdk::SP1Proof::Core(core_proofs) => {
                println!("Number of shards: {}", core_proofs.len());
                
                // Use SP1's standard bincode serialization approach (same as proof.save() uses internally)
                let proof_bytes = bincode::serialize(&proof).expect("failed to serialize proof");
                let proof_size_bytes = proof_bytes.len();
                let proof_size_kb = proof_size_bytes as f64 / 1024.0;
                let proof_size_mb = proof_size_kb / 1024.0;
                
                println!("Proof size: {} bytes ({:.2} KB, {:.3} MB)", 
                    proof_size_bytes, proof_size_kb, proof_size_mb);
                    
                // Calculate per-shard average size
                if !core_proofs.is_empty() {
                    let avg_shard_size = proof_size_bytes / core_proofs.len();
                    println!("Average shard size: {} bytes ({:.2} KB)", 
                        avg_shard_size, avg_shard_size as f64 / 1024.0);
                }
                
                // Optionally save proof to file using SP1's built-in save method
                if std::env::var("SP1_SAVE_PROOF").is_ok() {
                    let proof_path = "../proofs/core-proof.bin";
                    println!("Saving proof to: {}", proof_path);
                    if let Err(e) = proof.save(proof_path) {
                        println!("Warning: Failed to save proof: {}", e);
                    } else {
                        println!("Proof saved successfully");
                    }
                }
            }
            _ => println!("Unexpected proof type for core mode"),
        }
        
        // Print public values info
        let public_values_bytes = proof.public_values.as_slice();
        println!("Public values size: {} bytes", public_values_bytes.len());
        println!("Public values (hex): 0x{}", hex::encode(public_values_bytes));
        
        // Verify the proof
        println!("\nVerifying proof...");
        let verify_start = Instant::now();
        client.verify(&proof, &vk).expect("failed to verify proof");
        let verify_duration = verify_start.elapsed();
        
        println!("Successfully verified proof!");
        println!("Verification time: {:.2}s", verify_duration.as_secs_f64());
        
        // Print performance summary
        println!("\n=== Performance Summary ===");
        println!("Setup time:      {:.2}s", setup_duration.as_secs_f64());
        println!("Proving time:    {:.2}s", prove_duration.as_secs_f64());
        println!("Verification:    {:.2}s", verify_duration.as_secs_f64());
        println!("Total time:      {:.2}s", (setup_duration + prove_duration + verify_duration).as_secs_f64());
    }
}
