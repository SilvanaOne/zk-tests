module sum::main;

use sui::event;

public struct SumEvent has copy, drop {
    a: u64,
    b: u64,
    sum: u64,
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
