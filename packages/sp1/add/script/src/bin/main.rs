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
use alloy_sol_types::SolType;
use clap::Parser;
use sp1_sdk::{ProverClient, SP1Stdin, include_elf};
use std::time::Instant;

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

    #[arg(long, default_value = "10")]
    length: u32,

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
        // Setup the prover client.
        let client = ProverClient::from_env();
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
        // Setup the prover client.
        let client = ProverClient::from_env();
        let setup_start = Instant::now();
        let (pk, vk) = client.setup(ADD_ELF);
        let setup_duration = setup_start.elapsed();
        println!("Setup completed in {:.2}s", setup_duration.as_secs_f64());

        let proof = proof::generate_single_proof(&client, &pk, &stdin, FinalProofType::Core)
            .expect("failed to generate proof");
        proof::print_proof_statistics(&proof);

        let verify_duration =
            proof::verify_proof(&client, &proof, &vk).expect("failed to verify proof");

        // Print performance summary
        println!("\n=== Performance Summary ===");
        println!("Setup time:      {:.2}s", setup_duration.as_secs_f64());
        println!("Verification:    {:.2}s", verify_duration.as_secs_f64());
    } else {
        // Multiple proofs with aggregation
        let setup_start = Instant::now();
        let result = proof::generate_and_aggregate_proofs(
            ADD_ELF,
            AGGREGATE_ELF,
            args.proofs,
            args.length,
            FinalProofType::Core,
        )
        .expect("failed to generate and aggregate proofs");
        let setup_duration = setup_start.elapsed();

        proof::print_proof_statistics(&result.proof);
        proof::print_aggregated_results(&result.aggregated_values);
        proof::print_aggregation_performance_summary(setup_duration, &result);
    }
}
