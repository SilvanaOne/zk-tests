module bcs::main;

use std::string::String;
use sui::bcs::{Self, to_bytes};
use sui::event;

public struct UserRequest has copy, drop, store {
    name: String,
    data: u256,
    signature: vector<u8>,
}

public struct UserState has key, store {
    id: UID,
    name: String,
    data: u256,
    signature: vector<u8>,
    sequence: u64,
}

public struct UserStateData has copy, drop {
    name: String,
    data: u256,
    signature: vector<u8>,
    sequence: u64,
}

public struct UserStateEvent has copy, drop {
    name: String,
    data: u256,
    signature: vector<u8>,
    sequence: u64,
    serialized_state: vector<u8>,
}

// Debug event to track deserialization process
public struct DeserializeDebugEvent has copy, drop {
    step: String,
    input_size: u64,
    data_extracted: u256,
    signature_size: u64,
    remaining_bytes: u64,
}

// Debug event to show BCS serialization
public struct BcsDebugEvent has copy, drop {
    step: String,
    bcs_bytes: vector<u8>,
    bytes_length: u64,
    expected_data: u256,
    expected_signature: vector<u8>,
}

public fun create_user(
    name: String,
    data: u256,
    signature: vector<u8>,
    ctx: &mut TxContext,
) {
    let state = UserState {
        id: object::new(ctx),
        name,
        data,
        signature,
        sequence: 0,
    };
    transfer::share_object(state);
}

public fun user_request_1(
    state: &mut UserState,
    name: String,
    data: u256,
    signature: vector<u8>,
    _ctx: &mut TxContext,
) {
    state.sequence = state.sequence + 1;
    state.data = data;
    state.signature = signature;
    state.name = name;
    let serialized_state = to_bytes(
        &UserStateData {
            name: state.name,
            data: state.data,
            signature: state.signature,
            sequence: state.sequence,
        },
    );
    let event = UserStateEvent {
        name,
        data,
        signature,
        sequence: state.sequence,
        serialized_state: serialized_state,
    };
    event::emit(event);
}

fun user_request_2_internal(
    state: &mut UserState,
    user_request: UserRequest,
    _ctx: &mut TxContext,
) {
    state.sequence = state.sequence + 1;
    state.data = user_request.data;
    state.signature = user_request.signature;
    state.name = user_request.name;
    let serialized_state = to_bytes(
        &UserStateData {
            name: user_request.name,
            data: user_request.data,
            signature: user_request.signature,
            sequence: state.sequence,
        },
    );
    let event = UserStateEvent {
        name: user_request.name,
        data: user_request.data,
        signature: user_request.signature,
        sequence: state.sequence,
        serialized_state: serialized_state,
    };
    event::emit(event);
}

public fun user_request_2(
    state: &mut UserState,
    request: vector<u8>,
    _ctx: &mut TxContext,
) {
    // Emit debug event for the input
    event::emit(DeserializeDebugEvent {
        step: std::string::utf8(b"start_deserialization"),
        input_size: request.length(),
        data_extracted: 0,
        signature_size: 0,
        remaining_bytes: request.length(),
    });

    // Create BCS reader for manual deserialization using sui::bcs
    let mut bcs_reader = bcs::new(request);

    // Extract name (String is stored as ULEB128 length + UTF-8 bytes)
    let name_length = bcs_reader.peel_vec_length();
    let mut name_bytes = vector::empty<u8>();
    let mut i = 0u64;
    while (i < name_length) {
        vector::push_back(&mut name_bytes, bcs_reader.peel_u8());
        i = i + 1;
    };
    let name = std::string::utf8(name_bytes);

    // Extract u256 data (32 bytes little-endian)
    let data = bcs_reader.peel_u256();

    // Extract vector<u8> signature (ULEB128 length + bytes)
    let signature = bcs_reader.peel_vec_u8();

    // Check remaining bytes before consuming the reader
    let remaining = bcs_reader.into_remainder_bytes();
    let remaining_count = remaining.length();

    // Emit debug event showing what was extracted
    event::emit(DeserializeDebugEvent {
        step: std::string::utf8(b"extraction_complete"),
        input_size: request.length(),
        data_extracted: data,
        signature_size: signature.length(),
        remaining_bytes: remaining_count,
    });

    // Verify complete consumption (should be 0 remaining bytes)
    assert!(remaining_count == 0, 0x2001);

    // Create UserRequest from deserialized data
    let user_request = UserRequest { name, data, signature };

    // Call the internal function
    user_request_2_internal(state, user_request, _ctx);
}

// Public constructor for testing purposes
public fun new_user_request(
    name: String,
    data: u256,
    signature: vector<u8>,
): UserRequest {
    UserRequest { name, data, signature }
}
