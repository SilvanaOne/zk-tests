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
use add_script::proof::{self, FinalProofType};
use add_script::map::{AccountManager, AccountOperation};
use alloy_sol_types::{SolType, private::U256};
use clap::Parser;
use sp1_sdk::{ProverClient, SP1Stdin, include_elf};
use std::time::Instant;
use rand::{thread_rng, Rng};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const ADD_ELF: &[u8] = include_elf!("add-program");

/// The ELF for the aggregation program.
pub const AGGREGATE_ELF: &[u8] = include_elf!("aggregate-program");

/// The arguments for the command.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    execute: bool,

    #[arg(long)]
    prove: bool,

    #[arg(long, alias = "length", default_value = "10")]
    operations: u32,

    #[arg(long, default_value = "2")]
    proofs: u32,
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

    // Generate random account operations
    let mut rng = thread_rng();
    let operations: Vec<AccountOperation> = (0..args.operations)
        .map(|_| {
            let account_num = rng.gen_range(1..=100);
            let add_value = rng.gen_range(1..=1000);
            AccountOperation::new(account_num, add_value)
        })
        .collect();

    println!("Generated {} account operations", operations.len());

    // Create account manager and process operations
    let mut manager = AccountManager::new(16); 
    let initial_root = manager.get_root();
    
    // Setup the inputs for zkVM
    let mut stdin = SP1Stdin::new();
    
    // Write initial root as [u8; 32]
    let root_bytes: [u8; 32] = initial_root.as_bytes().try_into().unwrap();
    stdin.write(&root_bytes);
    
    // Write number of operations
    stdin.write(&args.operations);
    
    // Process operations and collect actions
    let mut actions = Vec::new();
    for op in &operations {
        let action = manager.process_action(op.account_num, op.add_value)
            .expect("Failed to process action");
        
        // Serialize the action for zkVM
        let action_bytes = borsh::to_vec(&action).expect("Failed to serialize action");
        stdin.write(&action_bytes);
        
        actions.push(action);
    }
    
    let final_root = manager.get_root();
    println!("\nInitial root: 0x{}", hex::encode(initial_root.as_bytes()));
    println!("Final root: 0x{}", hex::encode(final_root.as_bytes()));

    if args.execute {
        // Execute the program
        let client = ProverClient::from_env();
        let (output, report) = client.execute(ADD_ELF, &stdin).run().unwrap();
        println!("Program executed successfully.");

        // Read the output
        let output_bytes = output.as_slice();
        let decoded = PublicValuesStruct::abi_decode(output_bytes).unwrap();
        let PublicValuesStruct { old_root, new_root } = decoded;
        
        // Convert roots to U256 for comparison
        let old_root_bytes: [u8; 32] = initial_root.as_bytes().try_into().unwrap();
        let expected_old_root = U256::from_be_bytes(old_root_bytes);
        let new_root_bytes: [u8; 32] = final_root.as_bytes().try_into().unwrap();
        let expected_new_root = U256::from_be_bytes(new_root_bytes);
        
        println!("Old root matches: {}", old_root == expected_old_root);
        println!("New root matches: {}", new_root == expected_new_root);

        // Record the number of cycles executed
        println!("\nNumber of cycles: {}", report.total_instruction_count());
        println!("Number of syscalls: {}", report.total_syscall_count());
        println!(
            "Total instructions: {}",
            report.total_instruction_count() + report.total_syscall_count()
        );
        println!(
            "Touched memory addresses: {}",
            report.touched_memory_addresses
        );

        // Print detailed execution report
        println!("\n=== Detailed Execution Report ===");
        println!("{report}");
    } else if args.proofs == 1 {
        // Single proof mode
        let setup_start = Instant::now();
        
        let proof = proof::generate_single_proof(ADD_ELF, &stdin, FinalProofType::Core)
            .expect("failed to generate proof");
        proof::print_proof_statistics(&proof);

        // Get verification key for verification
        let client = ProverClient::from_env();
        let (_pk, vk) = client.setup(ADD_ELF);
        
        let verify_duration =
            proof::verify_proof(&proof, &vk).expect("failed to verify proof");
        
        let setup_duration = setup_start.elapsed();

        // Print performance summary
        println!("\n=== Performance Summary ===");
        println!("Total time:      {:.2}s", setup_duration.as_secs_f64());
        println!("Verification:    {:.2}s", verify_duration.as_secs_f64());
        
        // Save the core proof to JSON
        proof::save_core_proof(&proof, &vk).expect("failed to save core proof");
    } else {
        // Multiple proofs with aggregation
        let result = proof::generate_and_aggregate_proofs(
            ADD_ELF,
            AGGREGATE_ELF,
            args.proofs,
            args.operations,
            FinalProofType::Core,
        )
        .expect("failed to generate and aggregate proofs");

        proof::print_proof_statistics(&result.proof);
        proof::print_aggregated_results(&result.aggregated_values);
        proof::print_aggregation_performance_summary(&result);
        
        // Save the aggregated core proof to JSON
        proof::save_core_proof(&result.proof, &result.aggregate_vk).expect("failed to save aggregated core proof");
    }
}