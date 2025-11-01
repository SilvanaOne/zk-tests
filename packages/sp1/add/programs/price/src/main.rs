//! A program that verifies price proof data and commits verified price information
//! Takes PriceProofData, verifies all components, and outputs symbol, price, and timestamp

#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_sol_types::{sol, SolType};
use borsh::BorshDeserialize;
use price_lib::{verify_all, PriceProofData};

sol! {
    /// The public values encoded as a struct that can be easily deserialized inside Solidity.
    struct PublicValuesStruct {
        string symbol;
        string price;
        uint64 timestamp_fetched;
    }
}

pub fn main() {
    // 1. Read serialized PriceProofData from stdin
    let proof_bytes: Vec<u8> = sp1_zkvm::io::read();

    // 2. Deserialize using borsh
    let proof_data: PriceProofData = PriceProofData::deserialize(&mut &proof_bytes[..])
        .expect("Failed to deserialize PriceProofData");

    // 3. Verify all proof components
    let verification = verify_all(
        &proof_data.certificates,
        &proof_data.checkpoint,
        &proof_data.tsa_timestamp,
        proof_data.price.timestamp_fetched,
    )
    .expect("Verification failed");

    // 4. Assert all verifications passed
    assert!(
        verification.all_verified,
        "Verification failed: not all checks passed"
    );

    // 5. Extract price data for public values
    let symbol = proof_data.price.symbol;
    let price = proof_data.price.price;
    let timestamp_fetched = proof_data.price.timestamp_fetched;

    // 6. Encode public values for Solidity
    let bytes = PublicValuesStruct::abi_encode(&PublicValuesStruct {
        symbol,
        price,
        timestamp_fetched,
    });

    // 7. Commit to public values
    sp1_zkvm::io::commit_slice(&bytes);
}
