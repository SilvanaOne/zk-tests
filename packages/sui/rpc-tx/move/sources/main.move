module sum::main;

use std::ascii::String;
use sui::event;
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
}

public struct StateCreatedEvent has copy, drop {
    id: address,
    sum: u64,
    epoch: u64,
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
    let state = State { id, sum: 0 };
    transfer::share_object(state);
    let epoch = epoch(ctx);
    let event = StateCreatedEvent {
        id: state_address,
        sum: 0,
        epoch,
    };
    event::emit(event);
}

public fun add_to_state(
    state: &mut State,
    value: u64,
    epoch: u64,
    ctx: &mut TxContext,
) {
    let current_epoch = epoch(ctx);
    assert!(current_epoch == epoch, 1);
    let state_address = state.id.to_address();
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
    let state = State { id, sum: 0 };
    transfer::share_object(state);
    state_address
}
