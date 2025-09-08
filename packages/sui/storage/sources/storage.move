module storage::storage;

public struct Data has key, store {
    id: UID,
    value: vector<u8>,
}

public fun create(ctx: &mut TxContext) {
    let id = object::new(ctx);
    let value = vector<u8>[1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let data = Data { id, value };
    transfer::share_object(data);
}

public fun change(data: &mut Data) {
    data.value = vector<u8>[11, 12, 13, 14, 15, 16, 17, 18, 19, 20];
}
