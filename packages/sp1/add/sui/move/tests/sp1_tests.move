#[test_only]
module add_sp1::sp1_tests;

use add_sp1::main::verify_committed_values;
use std::hash;

#[test]
fun test_verify_committed_values_with_fixture_data() {
    // Test data: old_sum=0, new_sum=66 (sum of [10,11,12,13,14,15,16,17,18,19])
    let test_old_sum = 0u32;
    let test_new_sum = 145u32; // 10+11+12+13+14+15+16+17+18+19 = 145

    // Compute the actual digest for these values
    let expected_digest = compute_sp1_digest(
        test_old_sum,
        test_new_sum,
    );

    // Test the verification function
    let result = verify_committed_values(
        expected_digest,
        test_old_sum,
        test_new_sum,
    );

    // The function should return true for the correct values
    assert!(result, 0);
}

#[test]
fun test_verify_committed_values_with_wrong_values() {
    // Test with wrong values - should return false
    let test_old_sum = 0u32;
    let test_new_sum = 100u32; // Wrong new_sum

    // Compute the digest for the correct values (0, 145)
    let correct_digest = compute_sp1_digest(0u32, 145u32);

    // Test the verification function with wrong values but correct digest
    let result = verify_committed_values(
        correct_digest,
        test_old_sum,
        test_new_sum,
    );

    // The function should return false for wrong values
    assert!(!result, 1);
}

#[test]
fun test_verify_committed_values_with_wrong_digest() {
    // Test with correct values but wrong digest - should return false
    let test_old_sum = 0u32;
    let test_new_sum = 145u32;

    // Wrong digest (all zeros)
    let mut wrong_digest = std::vector::empty<u8>();
    let mut i = 0u64;
    while (i < 32) {
        std::vector::push_back(&mut wrong_digest, 0u8);
        i = i + 1;
    };

    // Test the verification function
    let result = verify_committed_values(
        wrong_digest,
        test_old_sum,
        test_new_sum,
    );

    // The function should return false for wrong digest
    assert!(!result, 2);
}

/// Encode a u32 value to ABI format (32 bytes, big-endian, right-padded)
fun encode_u32_to_abi_field(bytes: &mut vector<u8>, value: u32) {
    // ABI encoding pads u32 to 32 bytes, with the value in the last 4 bytes (big-endian)
    let mut i = 0u64;
    while (i < 28) {
        std::vector::push_back(bytes, 0u8); // Padding bytes
        i = i + 1;
    };

    // Add the 4 bytes of the u32 value (big-endian)
    std::vector::push_back(bytes, ((value >> 24) & 0xFF) as u8); // Most significant byte
    std::vector::push_back(bytes, ((value >> 16) & 0xFF) as u8);
    std::vector::push_back(bytes, ((value >> 8) & 0xFF) as u8);
    std::vector::push_back(bytes, (value & 0xFF) as u8); // Least significant byte
}

/// Compute digest using SP1's method: SHA256 hash with top 3 bits of first byte masked to 0
/// This matches the SP1PublicValues.hash_bn254() implementation in Rust
fun compute_sp1_digest(old_sum: u32, new_sum: u32): vector<u8> {
    // ABI-encode the values
    let mut encoded_bytes = std::vector::empty<u8>();
    encode_u32_to_abi_field(&mut encoded_bytes, old_sum);
    encode_u32_to_abi_field(&mut encoded_bytes, new_sum);

    // Hash with SHA256
    let mut hash = hash::sha2_256(encoded_bytes);

    // Mask the top 3 bits of the first byte (matches SP1's hash[0] &= 0b00011111)
    let first_byte = *std::vector::borrow(&hash, 0);
    let masked_byte = first_byte & 31u8; // Mask top 3 bits to 0 (31 = 0b00011111)
    *std::vector::borrow_mut(&mut hash, 0) = masked_byte;

    hash
}
