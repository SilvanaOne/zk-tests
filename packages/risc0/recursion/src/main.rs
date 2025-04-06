use balance::operation;
use balance_core::{State, StepOperation, StepPublicData, StepRequest};
use recursion_methods::{AGGREGATE_ELF, AGGREGATE_ID};
use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts, Receipt};
use std::time::Instant;

const COUNT: usize = 10;
const START_BALANCE: u64 = 0;
fn main() {
    let start_time_prover = Instant::now();
    let mut steps = Vec::<StepPublicData>::new();
    let mut operation_receipts = Vec::<Receipt>::new();
    let mut balance = START_BALANCE;
    for i in 0..COUNT {
        let add = (i + 1) as u64 * 2;
        let request = StepRequest {
            state: State {
                sequence: i as u64,
                balance,
            },
            operation: StepOperation { add },
        };
        let (operation_receipt, public_data) = operation(&request);
        steps.push(public_data);
        operation_receipts.push(operation_receipt);
        balance = balance + add;
    }

    let end_time_prover = Instant::now();
    println!("Prover time: {:?}", end_time_prover - start_time_prover);

    let merge_time_start = Instant::now();

    let mut env = ExecutorEnv::builder();
    for _ in 0..COUNT {
        env.add_assumption(operation_receipts.remove(0));
    }
    let circuit = env.write(&steps).unwrap().build().unwrap();

    let receipt = default_prover()
        .prove_with_opts(circuit, AGGREGATE_ELF, &ProverOpts::succinct())
        .unwrap()
        .receipt;
    let merge_time_end = Instant::now();
    println!("Merge time: {:?}", merge_time_end - merge_time_start);

    let verify_time_start = Instant::now();

    receipt.verify(AGGREGATE_ID).unwrap();
    let verify_time_end = Instant::now();
    println!("Verify time: {:?}", verify_time_end - verify_time_start);

    let json = serde_json::to_string(&receipt).unwrap();
    // Save the receipt to a file
    std::fs::write("receipt.json", json).expect("Failed to write receipt to file");

    // Decode the receipt to get (n, e, and c = x^e mod n).
    let data: StepPublicData = receipt.journal.decode().unwrap();

    println!("start sequence: {}", data.input.sequence);
    println!("end sequence: {}", data.output.sequence);
    println!("input: {}", data.input.balance);
    println!("output: {}", data.output.balance);
}
