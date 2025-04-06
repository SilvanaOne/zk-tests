use balance_core::{StepPublicData, StepRequest};
use balance_methods::OPERATION_ELF;
use risc0_zkvm::{default_prover, ExecutorEnv, Receipt};

pub fn operation(request: &StepRequest) -> (Receipt, StepPublicData) {
    let env = ExecutorEnv::builder()
        // Send a & b to the guest
        .write(&request)
        .unwrap()
        .build()
        .unwrap();

    // Obtain the default prover.
    let prover = default_prover();

    // Produce a receipt by proving the specified ELF binary.
    let receipt = prover.prove(env, OPERATION_ELF).unwrap().receipt;

    // Extract journal of receipt (i.e. output c, where c = a * b)
    let public_data: StepPublicData = receipt.journal.decode().expect(
        "Journal output should deserialize into the same types (& order) that it was written",
    );
    assert!(
        public_data.output.sequence == request.state.sequence + 1,
        "Sequence should be incremented by 1"
    );
    assert!(
        public_data.output.balance == request.state.balance + request.operation.add,
        "Balance should be the sum of the input and add"
    );

    // Report the product
    println!(
        "The balance for sequence {} is {}",
        public_data.output.sequence, public_data.output.balance
    );

    (receipt, public_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use balance_core::{State, StepOperation, StepRequest};

    #[test]
    fn test_operation() {
        const TEST_SEQUENCE: u64 = 17;
        const TEST_SUM: u64 = 23;
        const TEST_ADD: u64 = 10;

        let request = StepRequest {
            state: State {
                sequence: TEST_SEQUENCE,
                balance: TEST_SUM,
            },
            operation: StepOperation { add: TEST_ADD },
        };
        let (_, data) = operation(&request);
        assert_eq!(
            data.output.balance,
            TEST_SUM + TEST_ADD,
            "We expect the zkVM output to be the sum of the inputs"
        );
        assert_eq!(
            data.output.sequence,
            TEST_SEQUENCE + 1,
            "We expect the sequence to be incremented by 1"
        );
        assert_eq!(
            request.state, data.input,
            "We expect the input to be the same"
        );
    }
}
