module sum::main;

use std::ascii::String;
use sui::address;
use sui::bcs;
use sui::ed25519;
use sui::event;
use sui::hash;
use sui::package::{claim, published_package, published_module};
use sui::tx_context::epoch;

public struct SumEvent has copy, drop {
    a: u64,
    b: u64,
    sum: u64,
}

public struct State has key, store {
    id: sui::object::UID,
    sum: u64,
    owner: address,
}

public struct StateCreatedEvent has copy, drop {
    id: address,
    sum: u64,
    epoch: u64,
    owner: address,
}

public struct StateChangeEvent has copy, drop {
    old_sum: u64,
    new_sum: u64,
    state_address: address,
}

public struct PackagePublishedEvent has copy, drop {
    package_name: String,
    module_name: String,
}

public struct MAIN has drop {}

fun init(otw: MAIN, ctx: &mut TxContext) {
    let publisher = claim(otw, ctx);

    let package_name = published_package(&publisher);
    let module_name = published_module(&publisher);
    let event = PackagePublishedEvent {
        package_name: *package_name,
        module_name: *module_name,
    };
    event::emit(event);
    transfer::public_transfer(publisher, ctx.sender());
}

public fun calculate_sum(a: u64, b: u64): u64 {
    let sum = a + b;
    let event = SumEvent {
        a,
        b,
        sum,
    };
    event::emit(event);
    sum
}

public fun create_state(ctx: &mut TxContext) {
    let id = object::new(ctx);
    let state_address = id.to_address();
    let state = State { id, sum: 0, owner: ctx.sender() };
    transfer::share_object(state);
    let epoch = epoch(ctx);
    let event = StateCreatedEvent {
        id: state_address,
        sum: 0,
        epoch,
        owner: ctx.sender(),
    };
    event::emit(event);
}

/// Helper function to verify Ed25519 signature with embedded public key
/// Returns the signer's address if verification succeeds
fun verify_signature(
    signature_with_pk: &vector<u8>,
    message: &vector<u8>,
): address {
    // Signature format: 64 bytes signature + 32 bytes public key = 96 bytes total
    assert!(vector::length(signature_with_pk) == 96, 3); // Invalid signature format

    // Extract signature (first 64 bytes) and public key (last 32 bytes)
    let mut signature = vector::empty<u8>();
    let mut public_key = vector::empty<u8>();

    let mut i = 0;
    while (i < 64) {
        vector::push_back(
            &mut signature,
            *vector::borrow(signature_with_pk, i),
        );
        i = i + 1;
    };

    while (i < 96) {
        vector::push_back(
            &mut public_key,
            *vector::borrow(signature_with_pk, i),
        );
        i = i + 1;
    };

    // Verify the signature
    let verified = ed25519::ed25519_verify(
        &signature,
        &public_key,
        message,
    );
    assert!(verified, 1); // Signature verification failed

    // Derive the address from the public key
    // For Ed25519, we need to prepend the flag byte 0x00
    let mut pk_with_flag = vector::empty<u8>();
    vector::push_back(&mut pk_with_flag, 0x00); // Ed25519 flag
    vector::append(&mut pk_with_flag, public_key);
    let key_hash = hash::blake2b256(&pk_with_flag);
    address::from_bytes(key_hash)
}

public fun add_to_state(
    state: &mut State,
    value: u64,
    signature_with_pk: vector<u8>,
    ctx: &mut TxContext,
) {
    // Get current epoch and state address for signature verification
    let current_epoch = epoch(ctx);
    let state_address = state.id.to_address();

    // Create the message to verify: epoch (8 bytes) || state_address (32 bytes)
    let mut message = vector::empty<u8>();

    // Append epoch as 8 bytes (little-endian)
    let epoch_bytes = bcs::to_bytes(&current_epoch);
    vector::append(&mut message, epoch_bytes);

    // Append state address as 32 bytes
    let address_bytes = bcs::to_bytes(&state_address);
    vector::append(&mut message, address_bytes);

    // Verify signature and get signer's address
    let signer_address = verify_signature(&signature_with_pk, &message);
    assert!(signer_address == state.owner, 2); // Signer is not the owner

    // Update the state
    let old_sum = state.sum;
    state.sum = state.sum + value;
    let event = StateChangeEvent {
        old_sum,
        new_sum: state.sum,
        state_address,
    };
    event::emit(event);
}

public fun get_state(state: &State): u64 {
    state.sum
}

#[test_only]
public fun create_state_return_id_for_test(ctx: &mut TxContext): address {
    let id = object::new(ctx);
    let state_address = id.to_address();
    let state = State { id, sum: 0, owner: ctx.sender() };
    transfer::share_object(state);
    state_address
}
