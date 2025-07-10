#[test_only]
module bcs::bcs_tests;

use bcs::main;
use std::bcs as std_bcs;
use std::string::{Self, String};
use sui::test_scenario as test;

// Tests for the bcs::main module
//
// Test Coverage:
// ✅ create_user - Creates UserState objects with various inputs
// ✅ user_request_1 - Updates state with direct parameters
// ✅ user_request_2 - Now works with raw data components
//
// Note: All tests updated to match current function signatures with u256 and signature parameters

const ALICE: address = @0xa11ce;
const BOB: address = @0xb0b;

// Test helper function to create a test string
fun create_test_string(value: vector<u8>): String {
    string::utf8(value)
}

// Test helper function to create a test signature
fun create_test_signature(): vector<u8> {
    vector<u8>[0xaa, 0xbb, 0xcc, 0xdd]
}

#[test]
fun test_create_user() {
    let mut scenario = test::begin(ALICE);

    // Test creating a user - should not abort
    main::create_user(
        create_test_string(b"Alice"),
        42u256,
        create_test_signature(),
        test::ctx(&mut scenario),
    );

    test::end(scenario);
}

#[test]
fun test_user_request_1() {
    let mut scenario = test::begin(ALICE);

    // First create a user
    main::create_user(
        create_test_string(b"Alice"),
        10u256,
        create_test_signature(),
        test::ctx(&mut scenario),
    );

    test::next_tx(&mut scenario, BOB);

    // Test user_request_1 function - should not abort
    let mut state = test::take_shared<bcs::main::UserState>(&scenario);
    main::user_request_1(
        &mut state,
        create_test_string(b"Bob"),
        100u256,
        create_test_signature(),
        test::ctx(&mut scenario),
    );

    test::return_shared(state);
    test::end(scenario);
}

#[test]
fun test_user_request_1_multiple_calls() {
    let mut scenario = test::begin(ALICE);

    // First create a user
    main::create_user(
        create_test_string(b"Alice"),
        10u256,
        create_test_signature(),
        test::ctx(&mut scenario),
    );

    test::next_tx(&mut scenario, BOB);

    // Test calling user_request_1 multiple times - should not abort
    let mut state = test::take_shared<bcs::main::UserState>(&scenario);

    // First call
    main::user_request_1(
        &mut state,
        create_test_string(b"Charlie"),
        200u256,
        create_test_signature(),
        test::ctx(&mut scenario),
    );

    test::next_tx(&mut scenario, ALICE);

    // Second call
    main::user_request_1(
        &mut state,
        create_test_string(b"Updated"),
        300u256,
        create_test_signature(),
        test::ctx(&mut scenario),
    );

    test::return_shared(state);
    test::end(scenario);
}

#[test]
fun test_user_request_2() {
    let mut scenario = test::begin(ALICE);

    // First create a user
    main::create_user(
        create_test_string(b"Alice"),
        10u256,
        create_test_signature(),
        test::ctx(&mut scenario),
    );

    test::next_tx(&mut scenario, BOB);

    // Test user_request_2 function with BCS serialized data - should not abort
    let mut state = test::take_shared<bcs::main::UserState>(&scenario);

    // Create UserRequest and serialize it to BCS bytes
    let test_request = main::new_user_request(
        create_test_string(b"Bob Request 2"),
        500u256,
        vector<u8>[0x11, 0x22, 0x33, 0x44, 0x55],
    );
    let serialized_request = std_bcs::to_bytes(&test_request);

    // Call user_request_2 with serialized BCS bytes
    main::user_request_2(
        &mut state,
        serialized_request,
        test::ctx(&mut scenario),
    );

    test::return_shared(state);
    test::end(scenario);
}

#[test]
fun test_multiple_requests_sequence() {
    let mut scenario = test::begin(ALICE);

    // Create initial user
    main::create_user(
        create_test_string(b"Alice"),
        10u256,
        create_test_signature(),
        test::ctx(&mut scenario),
    );

    test::next_tx(&mut scenario, ALICE);

    // Make multiple requests to test that they execute without error
    let mut state = test::take_shared<bcs::main::UserState>(&scenario);

    // First request
    main::user_request_1(
        &mut state,
        create_test_string(b"Update1"),
        20u256,
        create_test_signature(),
        test::ctx(&mut scenario),
    );

    test::next_tx(&mut scenario, ALICE);

    // Second request
    main::user_request_1(
        &mut state,
        create_test_string(b"Update2"),
        30u256,
        create_test_signature(),
        test::ctx(&mut scenario),
    );

    test::next_tx(&mut scenario, ALICE);

    // Third request
    main::user_request_1(
        &mut state,
        create_test_string(b"Update3"),
        40u256,
        create_test_signature(),
        test::ctx(&mut scenario),
    );

    test::return_shared(state);
    test::end(scenario);
}

#[test]
fun test_empty_string_name() {
    let mut scenario = test::begin(ALICE);

    // Test with empty string name - should not abort
    main::create_user(
        create_test_string(b""),
        100u256,
        create_test_signature(),
        test::ctx(&mut scenario),
    );

    test::end(scenario);
}

#[test]
fun test_zero_request_value() {
    let mut scenario = test::begin(ALICE);

    // Test with zero request value - should not abort
    main::create_user(
        create_test_string(b"TestUser"),
        0u256,
        create_test_signature(),
        test::ctx(&mut scenario),
    );

    test::end(scenario);
}

#[test]
fun test_large_request_values() {
    let mut scenario = test::begin(ALICE);

    // Test with large request values - should not abort
    main::create_user(
        create_test_string(b"BigRequest"),
        18446744073709551615u256,
        create_test_signature(),
        test::ctx(&mut scenario),
    );

    test::end(scenario);
}
