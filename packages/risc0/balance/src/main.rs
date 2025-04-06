use balance::operation;
use balance_core::{State, StepOperation, StepRequest};
use balance_methods::OPERATION_ID;
//use risc0_zkvm::seal_to_json;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Pick two numbers
    let (receipt, _) = operation(&StepRequest {
        state: State {
            sequence: 10,
            balance: 23,
        },
        operation: StepOperation { add: 10 },
    });

    // Here is where one would send 'receipt' over the network...
    let json = serde_json::to_string(&receipt).unwrap();
    // Save the receipt to a file
    std::fs::write("receipt.json", json).expect("Failed to write receipt to file");

    println!("Receipt saved to receipt.json");
    // let seal = receipt
    //     .inner
    //     .groth16()
    //     .expect("Failed to get groth16 proof")
    //     .seal;

    // std::fs::write("seal.json", seal).expect("Failed to write seal to file");
    //let mut seal_file = std::fs::File::create("seal.json").expect("Failed to create seal file");
    //seal_to_json(seal, &mut seal_file).expect("Failed to write seal to JSON");
    //let mut seal_file = std::fs::File::create("seal.json").expect("Failed to create seal file");
    println!("Seal saved to seal.json");

    // Verify receipt, panic if it's wrong
    receipt.verify(OPERATION_ID).expect(
        "Code you have proven should successfully verify; did you specify the correct image ID?",
    );
}
