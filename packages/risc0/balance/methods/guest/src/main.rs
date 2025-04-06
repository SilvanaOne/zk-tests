#![no_main]
#![no_std]

use balance_core::{State, StepOperation, StepPublicData, StepRequest};
use risc0_zkvm::guest::env;

risc0_zkvm::guest::entry!(main);

fn main() {
    let step_request: StepRequest = env::read();
    let input: State = step_request.state;
    let operation: StepOperation = step_request.operation;
    if operation.add == 0 {
        panic!("Add number should be greater than 0")
    }

    let output = State {
        sequence: input.sequence + 1,
        balance: input.balance + operation.add,
    };
    let step_public_data = StepPublicData { input, output };
    env::commit(&step_public_data);
}
