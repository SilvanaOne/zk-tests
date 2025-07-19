module size::size;

public struct BigObject has key, store {
    id: UID,
    size: u64,
    data: vector<u256>,
}

public fun create(size: u64, ctx: &mut TxContext) {
    let mut i = 0;
    let mut data = vector::empty<u256>();
    while (i < size) {
        vector::push_back(&mut data, i as u256);
        i = i + 1;
    };
    let big_object = BigObject { id: object::new(ctx), size, data };
    transfer::share_object(big_object);
}

public fun run(size: u64): u256 {
    let mut i = 0;
    let mut data: u256 = 0;
    while (i < size) {
        data = data + 1;
        i = i + 1;
    };
    data
}
