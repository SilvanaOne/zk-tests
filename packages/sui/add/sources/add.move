/// Module: add
module add::add;

use sui::ecdsa_k1::secp256k1_verify;
use sui::event;

public struct Value has key, store {
    id: UID,
    sum: u64,
    actionsState: u256,
}

public struct AddEvent has copy, drop {
    sum: u64,
    actionsState: u256,
}

fun init(ctx: &mut TxContext) {
    let value = Value {
        id: object::new(ctx),
        sum: 0,
        actionsState: 0,
    };

    // Transfer the forge object to the module/package publisher
    transfer::transfer(value, ctx.sender());
}

public fun add_create(ctx: &mut TxContext): Value {
    Value {
        id: object::new(ctx),
        sum: 0,
        actionsState: 0,
    }
}

#[allow(lint(self_transfer))]
public fun create(ctx: &mut TxContext) {
    let value = Value {
        id: object::new(ctx),
        sum: 0,
        actionsState: 0,
    };
    transfer::transfer(value, ctx.sender());
}

#[error]
const EAmountTooLarge: vector<u8> = b"Amount too large";

public fun add(self: &mut Value, amount: u64): u64 {
    assert!(amount <= 10, EAmountTooLarge); // Check amount is less than or equal to 10
    self.sum = self.sum + amount;
    self.actionsState = self.actionsState + 1;
    event::emit(AddEvent {
        sum: self.sum,
        actionsState: self.actionsState,
    });
    self.sum
}

#[error]
const EInvalidSignature: vector<u8> = b"Invalid signature";

public fun add_signed(
    self: &mut Value,
    amount: u64,
    signature: vector<u8>,
    public_key: vector<u8>,
): u64 {
    let msg: vector<u8> = vector[1, 2, 3];
    let hash: u8 = 1;
    let valid = secp256k1_verify(&signature, &public_key, &msg, hash);
    std::debug::print(&valid);
    assert!(valid, EInvalidSignature);
    assert!(amount <= 10, EAmountTooLarge); // Check amount is less than or equal to 10
    self.sum = self.sum + amount;
    self.actionsState = self.actionsState + 1;
    event::emit(AddEvent {
        sum: self.sum,
        actionsState: self.actionsState,
    });
    self.sum
}

#[test]
fun test_init() {
    // Create a dummy TxContext for testing
    let mut ctx = tx_context::dummy();

    // Create a sword
    let value = Value {
        id: object::new(&mut ctx),
        sum: 0,
        actionsState: 0,
    };

    // Check if accessor functions return correct values
    assert!(value.sum == 0 && value.actionsState == 0, 1);
    let dummy_address = @0xCAFE;
    transfer::public_transfer(value, dummy_address);
}

#[test]
fun test_add() {
    // Create a dummy TxContext for testing
    let mut ctx = tx_context::dummy();

    // Create a sword
    let mut value = Value {
        id: object::new(&mut ctx),
        sum: 0,
        actionsState: 0,
    };

    add(&mut value, 10);
    add(&mut value, 5);

    // Check if accessor functions return correct values
    assert!(value.sum == 15 && value.actionsState == 2, 1);
    let dummy_address = @0xCAFE;
    transfer::public_transfer(value, dummy_address);
}

#[test]
fun test_add_signed() {
    // Create a dummy TxContext for testing
    let mut ctx = tx_context::dummy();

    // Create a sword
    let mut value = Value {
        id: object::new(&mut ctx),
        sum: 0,
        actionsState: 0,
    };

    let signature: vector<u8> = vector[
        99,
        237,
        151,
        116,
        234,
        74,
        25,
        36,
        164,
        69,
        181,
        141,
        222,
        64,
        141,
        224,
        166,
        255,
        245,
        159,
        184,
        5,
        193,
        229,
        206,
        57,
        228,
        130,
        12,
        100,
        137,
        64,
        103,
        133,
        69,
        161,
        25,
        202,
        201,
        223,
        102,
        205,
        84,
        30,
        162,
        182,
        172,
        255,
        24,
        196,
        178,
        4,
        196,
        215,
        87,
        239,
        157,
        164,
        224,
        132,
        168,
        154,
        80,
        8,
    ];
    let public_key = vector[
        3,
        228,
        135,
        106,
        12,
        84,
        227,
        5,
        206,
        208,
        117,
        50,
        124,
        155,
        224,
        44,
        1,
        192,
        183,
        132,
        137,
        196,
        154,
        181,
        104,
        6,
        17,
        30,
        47,
        28,
        64,
        93,
        236,
    ];

    add_signed(&mut value, 10, signature, public_key);
    add_signed(&mut value, 5, signature, public_key);

    // Check if accessor functions return correct values
    assert!(value.sum == 15 && value.actionsState == 2, 1);
    let dummy_address = @0xCAFE;
    transfer::public_transfer(value, dummy_address);
}

#[test]
fun test_add_transactions() {
    use sui::test_scenario;

    // Create test addresses representing users
    let initial_owner = @0xCAFE;
    let final_owner = @0xFACE;

    // First transaction executed by initial owner to create the sword
    let mut scenario = test_scenario::begin(initial_owner);
    {
        // Create the sword and transfer it to the initial owner
        let value = add_create(scenario.ctx());
        transfer::public_transfer(value, initial_owner);
    };

    // Second transaction executed by the initial sword owner
    scenario.next_tx(initial_owner);
    {
        // Extract the sword owned by the initial owner
        let mut value = scenario.take_from_sender<Value>();
        add(&mut value, 10);
        // Transfer the sword to the final owner
        transfer::public_transfer(value, final_owner);
    };

    // Third transaction executed by the final sword owner
    scenario.next_tx(final_owner);
    {
        // Extract the sword owned by the final owner
        let mut value = scenario.take_from_sender<Value>();
        add(&mut value, 5);
        // Verify that the sword has expected properties
        assert!(value.sum == 15 && value.actionsState == 2, 1);
        // Return the sword to the object pool (it cannot be simply "dropped")
        scenario.return_to_sender(value)
    };
    scenario.end();
}

#[test]
fun test_add_init() {
    use sui::test_scenario;

    // Create test addresses representing users
    let admin = @0xA675438;

    // First transaction to emulate module initialization
    let mut scenario = test_scenario::begin(admin);
    {
        init(scenario.ctx());
    };

    // Second transaction to check if the add has been created
    // and has initial value of zero
    scenario.next_tx(admin);
    {
        // Extract the Value object
        let value = scenario.take_from_sender<Value>();
        // Verify number of created swords
        assert!(value.sum == 0 && value.actionsState == 0, 1);
        // Return the Value object to the object pool
        scenario.return_to_sender(value);
    };

    // Third transaction executed by admin to create the value
    scenario.next_tx(admin);
    {
        let mut value = scenario.take_from_sender<Value>();
        // Create the add and transfer it to the initial owner
        add(&mut value, 10);
        scenario.return_to_sender(value);
    };
    scenario.end();
}
