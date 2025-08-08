#[test_only]
module add_sp1::sp1_tests;

use add_sp1::main::verify_committed_values;
use std::hash;

#[test]
fun test_verify_committed_values_with_fixture_data() {
    let test_old_root = 0u256;
    let test_new_root = 145u256;

    let expected_digest = compute_sp1_digest(test_old_root, test_new_root);

    let result = verify_committed_values(
        expected_digest,
        test_old_root,
        test_new_root,
    );

    assert!(result, 0);
}

#[test]
fun test_verify_committed_values_with_wrong_values() {
    let test_old_root = 0u256;
    let test_new_root = 100u256;

    let correct_digest = compute_sp1_digest(0u256, 145u256);

    let result = verify_committed_values(
        correct_digest,
        test_old_root,
        test_new_root,
    );

    assert!(!result, 1);
}

#[test]
fun test_verify_committed_values_with_wrong_digest() {
    let test_old_root = 0u256;
    let test_new_root = 145u256;

    let mut wrong_digest = std::vector::empty<u8>();
    let mut i = 0u64;
    while (i < 32) {
        std::vector::push_back(&mut wrong_digest, 0u8);
        i = i + 1;
    };

    let result = verify_committed_values(
        wrong_digest,
        test_old_root,
        test_new_root,
    );

    assert!(!result, 2);
}

fun encode_u256_to_abi_field(bytes: &mut vector<u8>, value: u256) {
    let mut tmp = value;
    let mut le = std::vector::empty<u8>();
    let mut i = 0u64;
    while (i < 32) {
        let byte = (tmp & 255u256) as u8;
        std::vector::push_back(&mut le, byte);
        tmp = tmp >> 8;
        i = i + 1;
    };
    let mut j = 32u64;
    while (j > 0) {
        j = j - 1;
        std::vector::push_back(bytes, *std::vector::borrow(&le, j));
    };
}

/// Compute digest using SP1's method: SHA256 hash with top 3 bits of first byte masked to 0
fun compute_sp1_digest(old_root: u256, new_root: u256): vector<u8> {
    let mut encoded_bytes = std::vector::empty<u8>();
    encode_u256_to_abi_field(&mut encoded_bytes, old_root);
    encode_u256_to_abi_field(&mut encoded_bytes, new_root);

    let mut hash = hash::sha2_256(encoded_bytes);
    let first_byte = *std::vector::borrow(&hash, 0);
    let masked_byte = first_byte & 31u8;
    *std::vector::borrow_mut(&mut hash, 0) = masked_byte;
    hash
}
