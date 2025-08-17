module sum::main;

use sui::event;

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
}

public struct StateChangeEvent has copy, drop {
    old_sum: u64,
    new_sum: u64,
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
    let event = StateCreatedEvent {
        id: state_address,
        sum: 0,
    };
    event::emit(event);
}

public fun add_to_state(state: &mut State, value: u64) {
    let old_sum = state.sum;
    state.sum = state.sum + value;
    let event = StateChangeEvent {
        old_sum,
        new_sum: state.sum,
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
