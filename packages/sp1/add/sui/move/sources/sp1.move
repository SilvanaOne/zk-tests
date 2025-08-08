/// Module: Add SP1 Verifier
/// This contract implements a simple example of verifying the proof of computing a Merkle root update
/// and maintaining a root state, similar to the Ethereum Add.sol contract but adapted for Sui.
module add_sp1::main;

use std::hash;
use sui::event;
use sui::groth16::{
    prepare_verifying_key,
    proof_points_from_bytes,
    public_proof_inputs_from_bytes,
    bn254,
    verify_groth16_proof
};

/// Struct representing the public values from the SP1 proof
#[allow(unused_field)]
public struct PublicValues has copy, drop {
    old_root: u256,
    new_root: u256,
}

/// The main Add contract object that maintains state
public struct AddContract has key, store {
    id: object::UID,
    /// The current root state
    root: u256,
    /// The hash of the add program (32 bytes)
    add_program_hash: vector<u8>,
}

/// Event emitted when root is updated
public struct RootUpdated has copy, drop {
    old_root: u256,
    new_root: u256,
}

//const EInvalidOldRoot: u64 = 1;
//const EInvalidComputation: u64 = 2;
const EProofVerificationFailed: u64 = 3;

/// Initialize a new Add contract
public fun create_contract(
    add_program_hash: vector<u8>,
    ctx: &mut tx_context::TxContext,
): AddContract {
    AddContract {
        id: object::new(ctx),
        root: 0, // Initialize root to 0
        add_program_hash,
    }
}

/// Create and immediately share a new Add contract
public fun create_and_share_contract(
    add_program_hash: vector<u8>,
    ctx: &mut tx_context::TxContext,
) {
    let contract = create_contract(add_program_hash, ctx);
    transfer::share_object(contract)
}

/// Verify that the committed values hash to the provided digest
/// This follows the SP1-Sui pattern where public values are committed via a digest
/// The function ABI-encodes the provided values and compares the hash with the digest
public fun verify_committed_values(
    committed_values_digest: vector<u8>,
    old_root: u256,
    new_root: u256,
): bool {
    // ABI-encode the public values (same as in the SP1 program)
    // Format: [32-byte old_root][32-byte new_root] (each u256 as 32 bytes)
    let mut encoded_bytes = std::vector::empty<u8>();

    // Encode old_root (32 bytes, big-endian)
    encode_u256_to_abi_field(&mut encoded_bytes, old_root);

    // Encode new_root (32 bytes, big-endian)
    encode_u256_to_abi_field(&mut encoded_bytes, new_root);

    // Hash the encoded bytes using SHA2-256 with top 3 bits masked (same as SP1 uses)
    let mut computed_digest = hash::sha2_256(encoded_bytes);

    // Mask the top 3 bits of the first byte (matches SP1's hash[0] &= 0b00011111)
    let first_byte = *std::vector::borrow(&computed_digest, 0);
    let masked_byte = first_byte & 31u8; // Mask top 3 bits to 0 (31 = 0b00011111)
    *std::vector::borrow_mut(&mut computed_digest, 0) = masked_byte;

    // Compare the computed digest with the provided digest
    computed_digest == committed_values_digest
}

/// Encode a u256 value to ABI format (32 bytes, big-endian)
fun encode_u256_to_abi_field(bytes: &mut vector<u8>, value: u256) {
    // ABI encoding uses full 32 bytes for u256 (big-endian)
    // Convert u256 to bytes in big-endian order
    let mut remaining = value;
    let mut result_bytes = std::vector::empty<u8>();

    // Extract 32 bytes from u256
    let mut i = 0u8;
    while (i < 32) {
        let byte_val = ((remaining & 0xFF) as u8);
        std::vector::push_back(&mut result_bytes, byte_val);
        remaining = remaining >> 8;
        i = i + 1;
    };

    // Reverse to get big-endian order (we extracted in little-endian)
    let mut j = 32u64;
    while (j > 0) {
        j = j - 1;
        std::vector::push_back(bytes, *std::vector::borrow(&result_bytes, j));
    };
}

/// The entrypoint for verifying the proof of a root update
/// Returns (old_root, new_root)
/// Follows SP1-Sui pattern: accepts system verification key and converted proof data
public fun verify_add_proof(
    contract: &mut AddContract,
    sp1_system_vkey: vector<u8>, // SP1 system verification key
    public_inputs: vector<u8>, // Converted public inputs (includes program hash)
    proof_points: vector<u8>, // Converted proof points
    old_root: u256, // Old root from the proof
    new_root: u256, // New root from the proof
): (u256, u256) {
    // Verify the Groth16 proof using Sui's native verification with SP1 system key
    let pvk = prepare_verifying_key(&bn254(), &sp1_system_vkey);
    let public_inputs_parsed = public_proof_inputs_from_bytes(public_inputs);
    let proof_points_parsed = proof_points_from_bytes(proof_points);

    // Verify the proof
    assert!(
        verify_groth16_proof(
            &bn254(),
            &pvk,
            &public_inputs_parsed,
            &proof_points_parsed,
        ),
        EProofVerificationFailed,
    );

    // Extract and validate program hash from public_inputs
    // SP1-Sui public inputs format: [vkey_hash (32 bytes), committed_values_digest (32 bytes)]
    // Each Fr element is serialized as 32 bytes in compressed format
    assert!(
        std::vector::length(&public_inputs) >= 64,
        EProofVerificationFailed,
    ); // 2 x 32 bytes

    // Extract the first 32 bytes as program hash
    let received_program_hash_raw = extract_bytes_range(&public_inputs, 0, 32);

    // SP1-Sui stores field elements in little-endian format, but our program hash is in big-endian
    // We need to reverse the byte order to match
    let received_program_hash = reverse_bytes(received_program_hash_raw);

    // Extract the committed values digest (second Fr element)
    let committed_values_digest_raw = extract_bytes_range(
        &public_inputs,
        32,
        32,
    );

    // SP1-Sui stores field elements in little-endian format, but our digest verification expects big-endian
    // We need to reverse the byte order to match
    let committed_values_digest = reverse_bytes(committed_values_digest_raw);

    // Validate that the program hash matches what we have stored
    assert!(
        received_program_hash == contract.add_program_hash,
        EProofVerificationFailed,
    );

    // Verify that the committed values match the provided values
    assert!(
        verify_committed_values(
            committed_values_digest,
            old_root,
            new_root,
        ),
        EProofVerificationFailed,
    );

    // Check that old_root matches current root state
    //assert!(public_values.old_root == contract.root, EInvalidOldRoot);

    // Update the root state
    contract.root = new_root;

    // Emit event with debug information
    event::emit(RootUpdated {
        old_root,
        new_root,
    });

    (old_root, new_root)
}

/// Get the current root
public fun get_current_root(contract: &AddContract): u256 {
    contract.root
}

/// Get the program hash
public fun get_program_hash(contract: &AddContract): vector<u8> {
    contract.add_program_hash
}

/// Extract a range of bytes from a vector
fun extract_bytes_range(
    bytes: &vector<u8>,
    start: u64,
    length: u64,
): vector<u8> {
    let mut result = std::vector::empty<u8>();
    let mut i = 0u64;
    while (i < length) {
        let byte = *std::vector::borrow(bytes, start + i);
        std::vector::push_back(&mut result, byte);
        i = i + 1;
    };
    result
}

/// Reverse the byte order of a vector
fun reverse_bytes(bytes: vector<u8>): vector<u8> {
    let mut result = std::vector::empty<u8>();
    let mut i = std::vector::length(&bytes);
    while (i > 0) {
        i = i - 1;
        std::vector::push_back(&mut result, *std::vector::borrow(&bytes, i));
    };
    result
}
