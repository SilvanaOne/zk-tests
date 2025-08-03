//! A simple program that takes a value and old_sum as input, and computes the addition
//! writing the result as output.

// These two lines are necessary for the program to properly compile.
//
// Under the hood, we wrap your main function with some extra code so that it behaves properly
// inside the zkVM.
#![no_main]
sp1_zkvm::entrypoint!(main);

use add_lib::{add_many, PublicValuesStruct};
use alloy_sol_types::SolType;

pub fn main() {
    // Read inputs to the program.
    //
    // Behind the scenes, this compiles down to a custom system call which handles reading inputs
    // from the prover.
    let old_sum = sp1_zkvm::io::read::<u32>();
    let num_values = sp1_zkvm::io::read::<u32>();

    // Read the array of values
    let mut values: Vec<u32> = Vec::new();
    for _ in 0..num_values {
        values.push(sp1_zkvm::io::read::<u32>());
    }

    // Compute the addition using add_many function from the workspace lib crate.
    let new_sum = add_many(&values, old_sum);

    // Encode the public values of the program.
    let bytes = PublicValuesStruct::abi_encode(&PublicValuesStruct { old_sum, new_sum });

    // Commit to the public values of the program. The final proof will have a commitment to all the
    // bytes that were committed to.
    sp1_zkvm::io::commit_slice(&bytes);
}
