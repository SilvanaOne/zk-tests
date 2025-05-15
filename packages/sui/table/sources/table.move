module table::table;

use sui::object_table;

public struct Element has key, store {
    id: UID,
    key: u256,
    value: u256,
}
public struct Table has key, store {
    id: UID,
    state: object_table::ObjectTable<u256, Element>,
}

public struct TABLE has drop {}

fun init(_otw: TABLE, ctx: &mut TxContext) {
    let table = Table {
        id: object::new(ctx),
        state: object_table::new<u256, Element>(ctx),
    };
    transfer::share_object(table);
}

public fun add(table: &mut Table, key: u256, value: u256, ctx: &mut TxContext) {
    table.state.add(key, Element { id: object::new(ctx), key, value });
}

public fun get(table: &Table, key: u256): u256 {
    table.state.borrow(key).value
}
