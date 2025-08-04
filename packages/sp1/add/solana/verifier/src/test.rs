use num_bigint::BigUint;
use num_traits::Num;
use sp1_sdk::SP1ProofWithPublicValues;

#[test]
fn test_verify_from_sp1() {
    use crate::{verify_proof, GROTH16_VK_5_0_0_BYTES};

    // Check if the groth16 proof file exists
    let sp1_proof_with_public_values_file = "../../../proofs/groth16-proof.bin";
    if !std::path::Path::new(sp1_proof_with_public_values_file).exists() {
        println!("Skipping test: groth16-proof.bin not found. Run the script to generate it first.");
        return;
    }

    // Read the serialized SP1ProofWithPublicValues from the file.
    let sp1_proof_with_public_values =
        SP1ProofWithPublicValues::load(&sp1_proof_with_public_values_file).unwrap();

    let proof_bytes = sp1_proof_with_public_values.bytes();
    let sp1_public_inputs = sp1_proof_with_public_values.public_values.to_vec();

    let proof = sp1_proof_with_public_values
        .proof
        .try_as_groth_16()
        .expect("Failed to convert proof to Groth16 proof");

    // Convert vkey hash to bytes.
    let vkey_hash = BigUint::from_str_radix(&proof.public_inputs[0], 10)
        .unwrap()
        .to_bytes_be();

    // To match the standard format, the 31 byte vkey hash is left padded with a 0 byte.
    let mut padded_vkey_hash = vec![0];
    padded_vkey_hash.extend_from_slice(&vkey_hash);
    let vkey_hash = padded_vkey_hash;

    let sp1_vkey_hash = format!("0x{}", hex::encode(vkey_hash));

    assert!(verify_proof(
        &proof_bytes,
        &sp1_public_inputs,
        &sp1_vkey_hash,
        &GROTH16_VK_5_0_0_BYTES
    )
    .is_ok());
}

#[test]
fn test_hash_public_inputs_() {
    use crate::utils::hash_public_inputs;

    // Check if the groth16 proof file exists
    let sp1_proof_with_public_values_file = "../../../proofs/groth16-proof.bin";
    if !std::path::Path::new(sp1_proof_with_public_values_file).exists() {
        println!("Skipping test: groth16-proof.bin not found. Run the script to generate it first.");
        return;
    }

    // Read the serialized SP1ProofWithPublicValues from the file.
    let sp1_proof_with_public_values =
        SP1ProofWithPublicValues::load(&sp1_proof_with_public_values_file).unwrap();

    let proof = sp1_proof_with_public_values
        .proof
        .try_as_groth_16()
        .expect("Failed to convert proof to Groth16 proof");

    let committed_values_digest = BigUint::from_str_radix(&proof.public_inputs[1], 10)
        .unwrap()
        .to_bytes_be();

    assert_eq!(
        committed_values_digest,
        hash_public_inputs(&sp1_proof_with_public_values.public_values.to_vec())
    );
}

#[test]
fn test_decode_sp1_vkey_hash() {
    use crate::utils::decode_sp1_vkey_hash;

    // Using the Add program's vkey hash from the lib.rs file
    let sp1_vkey_hash = "0x000118d2e7471764e0bbb6a5435391abd3e12fedda247326c7edb4f39c2ee928";
    let decoded_sp1_vkey_hash = decode_sp1_vkey_hash(sp1_vkey_hash).unwrap();
    assert_eq!(
        decoded_sp1_vkey_hash,
        hex_literal::hex!("000118d2e7471764e0bbb6a5435391abd3e12fedda247326c7edb4f39c2ee928")
    );
}
