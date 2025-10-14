//! Generate SP1 shrink proofs compatible with zkVerify.
//!
//! Shrink proofs are STARK proofs proven over a SNARK-friendly field,
//! suitable for verification on zkVerify without full SNARK wrapping.
//!
//! You can run this script using the following command:
//! ```shell
//! RUST_LOG=info cargo run --release --bin shrink -- --operations 10 --proofs 2
//! ```

use add_script::map::{AccountManager, AccountOperation};
use add_script::proof;
use clap::Parser;
use rand::{Rng, thread_rng};
use sp1_sdk::{ProverClient, SP1Stdin, include_elf};
use std::time::Instant;

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const ADD_ELF: &[u8] = include_elf!("add-program");
pub const AGGREGATE_ELF: &[u8] = include_elf!("aggregate-program");

/// The arguments for the shrink proof command.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct ShrinkArgs {
    #[arg(long, alias = "length", default_value = "10")]
    operations: u32,
    #[arg(long, default_value = "2")]
    proofs: u32,
    #[arg(long, default_value = "false")]
    hosted: bool,
}

fn main() {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();

    // Parse the command line arguments.
    let args = ShrinkArgs::parse();

    if args.proofs == 1 {
        // Single proof mode
        generate_single_shrink_proof(&args);
    } else {
        // Multiple proofs with aggregation
        generate_aggregated_shrink_proofs(&args);
    }
}

fn generate_single_shrink_proof(args: &ShrinkArgs) {
    let client = ProverClient::from_env();
    println!("Setting up proving keys...");
    let setup_start = Instant::now();
    let (_pk, vk) = client.setup(ADD_ELF);
    let setup_duration = setup_start.elapsed();
    println!("Setup completed in {:.2}s", setup_duration.as_secs_f64());

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
        let action = manager
            .process_action(op.account_num, op.add_value)
            .expect("Failed to process action");

        // Serialize the action for zkVM
        let action_bytes = borsh::to_vec(&action).expect("Failed to serialize action");
        stdin.write(&action_bytes);

        actions.push(action);
    }

    let final_root = manager.get_root();
    println!("\nInitial root: 0x{}", hex::encode(initial_root.as_bytes()));
    println!("Final root: 0x{}", hex::encode(final_root.as_bytes()));

    // Generate the shrink proof
    let (shrink_proof, public_values) = proof::generate_single_shrink_proof(ADD_ELF, &stdin)
        .expect("failed to generate shrink proof");

    // Print proof statistics
    proof::print_shrink_proof_statistics(&shrink_proof);

    // Verify the proof
    let verify_duration = proof::verify_shrink_proof(&shrink_proof, &vk)
        .expect("failed to verify shrink proof");

    // Print performance summary
    println!("\n=== Performance Summary ===");
    println!("Setup time:      {:.2}s", setup_duration.as_secs_f64());
    println!("Verification:    {:.2}s", verify_duration.as_secs_f64());

    // Create and save the fixture
    proof::save_shrink_proof(&shrink_proof, &vk, &public_values)
        .expect("failed to save shrink proof");
}

fn generate_aggregated_shrink_proofs(args: &ShrinkArgs) {
    let result = proof::generate_and_aggregate_shrink_proofs(
        ADD_ELF,
        AGGREGATE_ELF,
        args.proofs,
        args.operations,
        args.hosted,
    )
    .expect("failed to generate and aggregate shrink proofs");

    println!("Successfully generated aggregated shrink proof!");

    proof::print_shrink_proof_statistics(&result.shrink_proof);
    proof::print_aggregated_results(&result.aggregated_values);
    proof::print_aggregation_performance_summary_shrink(&result);

    // Save the aggregated shrink proof
    proof::save_shrink_proof(&result.shrink_proof, &result.aggregate_vk, &result.public_values)
        .expect("failed to save aggregated shrink proof");
}
