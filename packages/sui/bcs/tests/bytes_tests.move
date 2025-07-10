#[test_only]
module bcs::bytes_tests;

use bcs::main::{Self, UserRequest, new_user_request};
use std::bcs;
use std::debug;
use std::string;
use sui::bcs as sui_bcs;

#[test]
fun test_userrequest_bcs_round_trip() {
    // Create original UserRequest data using public constructor
    let original_name = string::utf8(b"Test User BCS");
    let original_data = 987654321u256;
    let original_signature = vector<u8>[11, 22, 33, 44, 55, 66];

    let original_request: UserRequest = new_user_request(
        original_name,
        original_data,
        original_signature,
    );

    // Serialize using std::bcs::to_bytes
    let serialized_bytes = bcs::to_bytes(&original_request);
    debug::print(&serialized_bytes);
    debug::print(&original_request);

    // Create BCS reader for deserialization using sui::bcs
    let mut bcs_reader = sui_bcs::new(serialized_bytes);

    // Extract name (String is stored as ULEB128 length + UTF-8 bytes)
    let name_length = bcs_reader.peel_vec_length();
    let mut name_bytes = vector::empty<u8>();
    let mut i = 0u64;
    while (i < name_length) {
        vector::push_back(&mut name_bytes, bcs_reader.peel_u8());
        i = i + 1;
    };
    let deserialized_name = string::utf8(name_bytes);

    // Extract u256 data (32 bytes little-endian)
    let deserialized_data = bcs_reader.peel_u256();

    // Extract vector<u8> signature (ULEB128 length + bytes)
    let deserialized_signature = bcs_reader.peel_vec_u8();

    // Compare original vs deserialized data
    assert!(deserialized_name == original_name, 0);
    assert!(deserialized_data == original_data, 1);
    assert!(deserialized_signature == original_signature, 2);

    // Verify no remaining bytes after complete deserialization
    let remaining = bcs_reader.into_remainder_bytes();
    assert!(vector::is_empty(&remaining), 3);
}

#[test]
fun test_rust_bcs_deserialization() {
    // This is the exact BCS bytes produced by Rust (53 bytes)
    // From Rust test output: 0d54657374205573657220424353b168de3a00000000000000000000000000000000000000000000000000000000060b16212c3742
    let rust_bcs_hex_bytes = vector<u8>[
        // String "Test User BCS" (length 0x0d + content)
        0x0d,
        0x54,
        0x65,
        0x73,
        0x74,
        0x20,
        0x55,
        0x73,
        0x65,
        0x72,
        0x20,
        0x42,
        0x43,
        0x53,
        // U256 value 987654321 (32 bytes little-endian)
        0xb1,
        0x68,
        0xde,
        0x3a,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        // Vector<u8> [11, 22, 33, 44, 55, 66] (length 0x06 + content)
        0x06,
        0x0b,
        0x16,
        0x21,
        0x2c,
        0x37,
        0x42,
    ];

    debug::print(&string::utf8(b"=== Move Deserializing Rust BCS Bytes ==="));
    debug::print(&rust_bcs_hex_bytes);

    // Create BCS reader for manual deserialization using sui::bcs
    let mut bcs_reader = sui_bcs::new(rust_bcs_hex_bytes);

    // Extract name (String is stored as ULEB128 length + UTF-8 bytes)
    let name_length = bcs_reader.peel_vec_length();
    let mut name_bytes = vector::empty<u8>();
    let mut i = 0u64;
    while (i < name_length) {
        vector::push_back(&mut name_bytes, bcs_reader.peel_u8());
        i = i + 1;
    };
    let deserialized_name = string::utf8(name_bytes);

    // Extract u256 data (32 bytes little-endian)
    let deserialized_data = bcs_reader.peel_u256();

    // Extract vector<u8> signature (ULEB128 length + bytes)
    let deserialized_signature = bcs_reader.peel_vec_u8();

    debug::print(
        &string::utf8(b"Successfully deserialized from Rust BCS bytes:"),
    );
    debug::print(&deserialized_name);
    debug::print(&deserialized_data);
    debug::print(&deserialized_signature);

    // Create expected values
    let expected_name = string::utf8(b"Test User BCS");
    let expected_data = 987654321u256;
    let expected_signature = vector<u8>[11, 22, 33, 44, 55, 66];

    debug::print(&string::utf8(b"Expected values:"));
    debug::print(&expected_name);
    debug::print(&expected_data);
    debug::print(&expected_signature);

    // Assert that deserialized values match original values
    assert!(deserialized_name == expected_name, 0x1001);
    assert!(deserialized_data == expected_data, 0x1002);
    assert!(deserialized_signature == expected_signature, 0x1003);

    // Verify no remaining bytes after complete deserialization
    let remaining = bcs_reader.into_remainder_bytes();
    assert!(vector::is_empty(&remaining), 0x1004);

    debug::print(&string::utf8(b"✅ Rust BCS deserialization successful!"));
    debug::print(
        &string::utf8(b"✅ Cross-language BCS compatibility verified!"),
    );

    // Re-serialize and verify we get the same bytes
    let reconstructed_request = new_user_request(
        deserialized_name,
        deserialized_data,
        deserialized_signature,
    );
    let reconstructed_bcs = bcs::to_bytes(&reconstructed_request);

    debug::print(&string::utf8(b"Original Rust bytes:"));
    debug::print(&rust_bcs_hex_bytes);
    debug::print(&string::utf8(b"Reconstructed bytes:"));
    debug::print(&reconstructed_bcs);

    // Verify round-trip consistency
    assert!(reconstructed_bcs == rust_bcs_hex_bytes, 0x1005);

    debug::print(&string::utf8(b"✅ Round-trip BCS serialization verified!"));
}

#[test]
fun test_bcs_format_analysis() {
    // Test to analyze the BCS format structure
    let test_name = string::utf8(b"Hello");
    let test_data = 300u256;
    let test_signature = vector<u8>[1, 2, 3];

    let test_request = main::new_user_request(
        test_name,
        test_data,
        test_signature,
    );

    // Serialize and check the byte structure
    let bcs_bytes = bcs::to_bytes(&test_request);
    let total_length = vector::length(&bcs_bytes);

    // Expected structure:
    // - String "Hello": 1 byte (length=5) + 5 bytes (UTF-8) = 6 bytes
    // - u256 300: 32 bytes
    // - vector [1,2,3]: 1 byte (length=3) + 3 bytes = 4 bytes
    // Total: 6 + 32 + 4 = 42 bytes
    assert!(total_length == 42, 4);

    // Test partial deserialization
    let mut reader = sui_bcs::new(bcs_bytes);

    // Read string length
    let str_len = reader.peel_vec_length();
    assert!(str_len == 5, 5);

    // Read string bytes
    let mut str_bytes = vector::empty<u8>();
    let mut i = 0u64;
    while (i < str_len) {
        vector::push_back(&mut str_bytes, reader.peel_u8());
        i = i + 1;
    };
    let name = string::utf8(str_bytes);
    assert!(name == test_name, 6);

    // Read u256
    let data = reader.peel_u256();
    assert!(data == test_data, 7);

    // Read vector length
    let vec_len = reader.peel_vec_length();
    assert!(vec_len == 3, 8);

    // Read vector bytes one by one
    let byte1 = reader.peel_u8();
    let byte2 = reader.peel_u8();
    let byte3 = reader.peel_u8();
    assert!(byte1 == 1 && byte2 == 2 && byte3 == 3, 9);

    // Verify complete consumption
    let leftover = reader.into_remainder_bytes();
    assert!(vector::is_empty(&leftover), 10);
}
