use balance_core::StepPublicData;
use balance_methods::OPERATION_ID;
use risc0_zkvm::guest::env;

fn main() {
    let steps: Vec<StepPublicData> = env::read();
    let count = steps.len();

    if count == 0 {
        panic!("No steps provided");
    }

    // Verify all steps
    for step in &steps {
        env::verify(OPERATION_ID, &step.to_vec()).unwrap();
    }

    // Check continuity between steps
    for i in 0..count - 1 {
        let prev = &steps[i];
        let next = &steps[i + 1];

        assert!(
            prev.output.sequence == next.input.sequence,
            "Output sequence should be equal to input sequence"
        );
        assert!(
            prev.output.balance == next.input.balance,
            "Output balance should be equal to input balance"
        );
    }

    env::commit(&StepPublicData {
        input: steps[0].input,
        output: steps[count - 1].output,
    });
}
