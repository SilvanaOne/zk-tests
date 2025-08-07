//! Hash computation example using SP1 SDK
//!
//! You can run this script using the following command:
//! ```shell
//! RUST_LOG=info cargo run --release --bin hash -- --execute --type sha256 --iterations 10
//! ```
//! or
//! ```shell
//! RUST_LOG=info cargo run --release --bin hash -- --prove --type sha256 --iterations 10
//! ```

use clap::Parser;
use sp1_sdk::{Prover, ProverClient, SP1Stdin, include_elf};
use proof_lib::create_poseidon_proof;

/// The ELF files for different hash programs
pub const SHA256_ELF: &[u8] = include_elf!("sha256-program");
// TODO: Add other hash ELFs when implemented
// pub const BN254_ELF: &[u8] = include_elf!("bn254-program");
pub const MINA_ELF: &[u8] = include_elf!("mina-program");
pub const P3_ELF: &[u8] = include_elf!("p3-program");
pub const PS_ELF: &[u8] = include_elf!("ps-program");
pub const PROOF_ELF: &[u8] = include_elf!("proof-program");

/// Supported hash types
#[derive(Parser, Debug, Clone)]
enum HashType {
    Sha256,
    Bn254,
    Mina,
    P3,
    PS,
    Proof,
}

impl std::str::FromStr for HashType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "sha256" => Ok(HashType::Sha256),
            "bn254" => Ok(HashType::Bn254),
            "mina" => Ok(HashType::Mina),
            "p3" => Ok(HashType::P3),
            "ps" => Ok(HashType::PS),
            "proof" => Ok(HashType::Proof),
            _ => Err(format!("Unknown hash type: {}", s)),
        }
    }
}

/// The arguments for the command.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    execute: bool,

    #[arg(long)]
    prove: bool,

    #[arg(long, default_value = "sha256")]
    r#type: HashType,

    #[arg(long, default_value = "10")]
    iterations: u32,
}

/// Create and serialize a real Poseidon proof
fn create_real_serialized_proof() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    println!("Creating Poseidon proof...");
    let proof = create_poseidon_proof()?;
    
    println!("Serializing proof...");
    let serialized = proof.serialize()?;
    println!("Serialized proof size: {} bytes", serialized.len());
    
    Ok(serialized)
}

fn main() {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();

    // Parse the command line arguments.
    let args = Args::parse();

    if args.execute == args.prove {
        eprintln!("Error: You must specify either --execute or --prove");
        std::process::exit(1);
    }

    // Select the appropriate ELF based on hash type
    let elf = match args.r#type {
        HashType::Sha256 => SHA256_ELF,
        HashType::Bn254 => {
            eprintln!("Error: BN254 hash is not yet implemented");
            std::process::exit(1);
        }
        HashType::Mina => MINA_ELF,
        HashType::P3 => P3_ELF,
        HashType::PS => PS_ELF,
        HashType::Proof => PROOF_ELF,
    };

    // Setup the prover client.
    let client = ProverClient::builder().cpu().build();

    // Setup the inputs.
    let mut stdin = SP1Stdin::new();

    // For Proof type, we need to prepare a serialized proof
    if matches!(args.r#type, HashType::Proof) {
        // Create a real serialized Poseidon proof
        let real_proof = create_real_serialized_proof()
            .expect("Failed to create Poseidon proof");
        stdin.write(&real_proof);
    } else {
        // Write the number of iterations
        stdin.write(&args.iterations);

        // Write the input data to hash
        let input_data: Vec<u32> = vec![1, 2, 3];
        stdin.write(&input_data);
    }

    println!("Hash type: {:?}", args.r#type);
    if !matches!(args.r#type, HashType::Proof) {
        println!("Iterations: {}", args.iterations);
        println!("Input data: [1, 2, 3]");
    } else {
        println!("Verifying serialized proof...");
    }

    if args.execute {
        // Execute the program
        let (output, report) = client.execute(elf, &stdin).run().unwrap();
        println!("Program executed successfully.");

        // Read the output based on type
        if matches!(args.r#type, HashType::Proof) {
            // For proof verification, we get verification result, error code, and proof size
            let output_bytes = output.as_slice();
            let verification_result: bool = output_bytes.get(0).map_or(false, |&b| b != 0);
            let error_code = if output_bytes.len() >= 5 {
                u32::from_le_bytes([
                    output_bytes[1],
                    output_bytes[2],
                    output_bytes[3],
                    output_bytes[4]
                ])
            } else {
                0
            };
            let proof_size = if output_bytes.len() >= 9 {
                u32::from_le_bytes([
                    output_bytes[5],
                    output_bytes[6],
                    output_bytes[7],
                    output_bytes[8]
                ])
            } else {
                0
            };
            println!("Deserialization result: {}", verification_result);
            println!("Error code: {} (0=success, 1=deserialize failed)", error_code);
            println!("Proof size: {} bytes", proof_size);
        } else {
            // Read the output digest
            println!("Hash digest: 0x{}", hex::encode(output.as_slice()));
        }

        // Record the number of cycles executed.
        println!("Number of cycles: {}", report.total_instruction_count());
        println!("Number of syscalls: {}", report.total_syscall_count());
        println!(
            "Total instructions: {}",
            report.total_instruction_count() + report.total_syscall_count()
        );

        // Print detailed execution report
        println!("\n=== Detailed Execution Report ===");
        println!("{report}");
    } else {
        // Generate proof
        println!("Generating proof...");
        let (pk, vk) = client.setup(elf);

        let prove_start = std::time::Instant::now();
        let proof = client
            .prove(&pk, &stdin)
            //.groth16()
            //.strategy(FulfillmentStrategy::Hosted)
            .run()
            .expect("failed to generate proof");
        let prove_duration = prove_start.elapsed();

        println!("Proof generated successfully!");
        println!("Proving time: {:.2}s", prove_duration.as_secs_f64());
        println!(
            "Average time per iteration: {:.2}ms",
            prove_duration.as_millis() as f64 / args.iterations as f64
        );

        // Verify the proof
        let verify_start = std::time::Instant::now();
        client.verify(&proof, &vk).expect("failed to verify proof");
        let verify_duration = verify_start.elapsed();

        println!("\n=== Verification ===");
        println!("Proof verified successfully!");
        println!("Verification time: {:.2}s", verify_duration.as_secs_f64());

        // Read the output from the proof
        println!("\n=== Output ===");
        if matches!(args.r#type, HashType::Proof) {
            // For proof verification, interpret the output differently
            let public_values_bytes = proof.public_values.as_slice();
            if !public_values_bytes.is_empty() {
                let verification_result: bool = public_values_bytes[0] != 0;
                let error_code = if public_values_bytes.len() >= 5 {
                    u32::from_le_bytes([
                        public_values_bytes[1],
                        public_values_bytes[2],
                        public_values_bytes[3],
                        public_values_bytes[4]
                    ])
                } else {
                    0
                };
                let proof_size = if public_values_bytes.len() >= 9 {
                    u32::from_le_bytes([
                        public_values_bytes[5],
                        public_values_bytes[6],
                        public_values_bytes[7],
                        public_values_bytes[8]
                    ])
                } else {
                    0
                };
                println!("Deserialization result: {}", verification_result);
                println!("Error code: {} (0=success, 1=deserialize failed)", error_code);
                println!("Proof size: {} bytes", proof_size);
            }
        } else {
            println!("Hash digest: 0x{}", hex::encode(proof.public_values.as_slice()));
        }
    }
}
