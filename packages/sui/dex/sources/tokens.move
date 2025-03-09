module dex::tokens;

use std::string::String;
use sui::ecdsa_k1::secp256k1_verify;
use sui::event;

public struct Token has key, store {
    id: UID,
    tokenId: u256,
    name: String,
    sum: u64,
    values: vector<u64>,
}

public struct TokenReference has store {
    id: ID,
    address: address,
    tokenId: u256,
    name: String,
    sum: u64,
    values: vector<u64>,
}

public struct DEX has key, store {
    id: UID,
    actionsState: u256,
    tokens: vector<TokenReference>,
}

public struct AddEvent has copy, drop {
    name: String,
    tokenId: u256,
    sum: u64,
    actionsState: u256,
    elements: vector<u64>,
}

public struct ResultEvent has copy, drop {
    result: u256,
    memo: String,
}

fun init(ctx: &mut TxContext) {
    let dex = DEX {
        id: object::new(ctx),
        actionsState: 0,
        tokens: vector[],
    };

    transfer::share_object(dex);
}

public fun create_token(dex: &mut DEX, tokenId: u256, name: String, ctx: &mut TxContext): address {
    let token = Token {
        id: object::new(ctx),
        tokenId,
        name,
        sum: 0,
        values: vector[],
    };
    let address = token.id.to_address();
    let tokenReference = TokenReference {
        id: object::uid_to_inner(&token.id),
        address,
        tokenId,
        name,
        sum: 0,
        values: vector[],
    };
    dex.tokens.push_back(tokenReference);
    transfer::share_object(token);
    address
}

// public fun add_create_transfer(address: address, ctx: &mut TxContext) {
//     let Token = Token {
//         id: object::new(ctx),
//         sum: 0,
//         actionsState: 0,
//         Tokens: vector[],
//     };
//     transfer::transfer(Token, address);
// }

// #[allow(lint(self_transfer))]
// public fun create(ctx: &mut TxContext) {
//     let Token = Token {
//         id: object::new(ctx),
//         sum: 0,
//         actionsState: 0,
//         Tokens: vector[],
//     };

//     transfer::transfer(Token, ctx.sender());
// }

#[error]
const EAmountTooLarge: vector<u8> = b"Amount too large";

public fun add(dex: &mut DEX, token: &mut Token, amount: u64): u64 {
    assert!(amount <= 10, EAmountTooLarge); // Check amount is less than or equal to 10
    token.sum = token.sum + amount;
    dex.actionsState = dex.actionsState + 1;
    token.values.push_back(amount);
    event::emit(AddEvent {
        name: token.name,
        tokenId: token.tokenId,
        sum: token.sum,
        actionsState: dex.actionsState,
        elements: token.values,
    });
    //std::debug::print(&self.sum);
    token.sum
}

public fun get_state(dex: &DEX): u256 {
    let state = dex.actionsState;
    std::debug::print(&b"State: ");
    std::debug::print(&state);
    let result = state * 2;
    std::debug::print(&b"Result: ");
    std::debug::print(&result);
    event::emit(ResultEvent {
        result,
        memo: b"This is a memo".to_string(),
    });
    result
}

#[error]
const EInvalidSignature: vector<u8> = b"Invalid signature";

public fun add_signed(
    dex: &mut DEX,
    token: &mut Token,
    amount: u64,
    signature: vector<u8>,
    public_key: vector<u8>,
): u64 {
    let msg: vector<u8> = vector[1, 2, 3];
    let hash: u8 = 1;
    let valid = secp256k1_verify(&signature, &public_key, &msg, hash);
    assert!(valid, EInvalidSignature);
    assert!(amount <= 10, EAmountTooLarge); // Check amount is less than or equal to 10
    token.sum = token.sum + amount;
    dex.actionsState = dex.actionsState + 1;
    token.values.push_back(amount);
    event::emit(AddEvent {
        name: token.name,
        tokenId: token.tokenId,
        sum: token.sum,
        actionsState: dex.actionsState,
        elements: token.values,
    });
    token.sum
}

#[test]
fun test_init() {
    // Create a dummy TxContext for testing
    let mut ctx = tx_context::dummy();

    // Create a sword
    let dex = DEX {
        id: object::new(&mut ctx),
        actionsState: 0,
        tokens: vector[],
    };

    // Check if accessor functions return correct Tokens
    assert!(dex.tokens.length() == 0, 1);
    let dummy_address = @0xCAFE;
    transfer::public_transfer(dex, dummy_address);
}

#[test]
fun test_add() {
    // Create a dummy TxContext for testing
    let mut ctx = tx_context::dummy();

    // Create a sword
    let mut dex = DEX {
        id: object::new(&mut ctx),
        actionsState: 0,
        tokens: vector[],
    };

    create_token(&mut dex, 10, b"test1".to_string(), &mut ctx);
    create_token(&mut dex, 5, b"test2".to_string(), &mut ctx);

    // add(&mut dex, & mut token1, 5);
    // add(&mut dex, token2, 10);

    // // Check if accessor functions return correct Tokens
    // assert!(dex.tokens.length() == 2, 1);
    let dummy_address = @0xCAFE;
    transfer::public_transfer(dex, dummy_address);
}

#[test]
fun test_add_signed() {
    // Create a dummy TxContext for testing
    let mut ctx = tx_context::dummy();

    let mut dex = DEX {
        id: object::new(&mut ctx),
        actionsState: 0,
        tokens: vector[],
    };

    create_token(&mut dex, 10, b"test1".to_string(), &mut ctx);
    create_token(&mut dex, 5, b"test2".to_string(), &mut ctx);

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

    let mut token = Token {
        id: object::new(&mut ctx),
        tokenId: 10,
        name: b"test1".to_string(),
        sum: 0,
        values: vector[],
    };

    add_signed(&mut dex, &mut token, 10, signature, public_key);
    add_signed(&mut dex, &mut token, 5, signature, public_key);

    // Check if accessor functions return correct Tokens
    assert!(dex.tokens.length() == 2, 1);
    let dummy_address = @0xCAFE;
    transfer::public_transfer(dex, dummy_address);
    transfer::public_transfer(token, dummy_address);
}

// #[test]
// fun test_add_create_transfer() {
//     // Create a dummy TxContext for testing
//     let mut ctx = tx_context::dummy();
//     let dummy_address = @0xCAFE;
//     add_create_transfer(dummy_address, &mut ctx);
// }

// #[test]
// fun test_add_transactions() {
//     use sui::test_scenario;

//     // Create test addresses representing users
//     let initial_owner = @0xCAFE;
//     let final_owner = @0xFACE;

//     // First transaction executed by initial owner to create the sword
//     let mut scenario = test_scenario::begin(initial_owner);
//     {
//         // Create the sword and transfer it to the initial owner
//         init(scenario.ctx());
//         transfer::public_transfer(Tokens, initial_owner);
//     };

//     // Second transaction executed by the initial sword owner
//     scenario.next_tx(initial_owner);
//     {
//         // Extract the sword owned by the initial owner
//         let mut Token = scenario.take_from_sender<Token>();
//         add(&mut Token, 10);
//         // Transfer the sword to the final owner
//         transfer::public_transfer(Token, final_owner);
//     };

//     // Third transaction executed by the final sword owner
//     scenario.next_tx(final_owner);
//     {
//         // Extract the sword owned by the final owner
//         let mut Token = scenario.take_from_sender<Token>();
//         add(&mut Token, 5);
//         // Verify that the sword has expected properties
//         assert!(Token.sum == 15 && Token.actionsState == 2, 1);
//         // Return the sword to the object pool (it cannot be simply "dropped")
//         scenario.return_to_sender(Token)
//     };
//     scenario.end();
// }

// #[test]
// fun test_add_init() {
//     use sui::test_scenario;

//     // Create test addresses representing users
//     let admin = @0xA675438;

//     // First transaction to emulate module initialization
//     let mut scenario = test_scenario::begin(admin);
//     {
//         init(scenario.ctx());
//     };

//     // Second transaction to check if the add has been created
//     // and has initial Token of zero
//     scenario.next_tx(admin);
//     {
//         // Extract the Token object
//         let Token = scenario.take_from_sender<Token>();
//         // Verify number of created swords
//         assert!(Token.sum == 0 && Token.actionsState == 0, 1);
//         // Return the Token object to the object pool
//         scenario.return_to_sender(Token);
//     };

//     // Third transaction executed by admin to create the Token
//     scenario.next_tx(admin);
//     {
//         let mut Token = scenario.take_from_sender<Token>();
//         // Create the add and transfer it to the initial owner
//         add(&mut Token, 10);
//         scenario.return_to_sender(Token);
//     };
//     scenario.end();
// }

// #[test]
// fun test_add_start() {
//     use sui::test_scenario;

//     // Create test addresses representing users
//     let admin = @0xA675438;

//     // First transaction to emulate module initialization
//     let mut scenario = test_scenario::begin(admin);
//     {
//         add_create_transfer(admin, scenario.ctx());
//     };

//     // Second transaction to check if the add has been created
//     // and has initial Token of zero
//     scenario.next_tx(admin);
//     {
//         // Extract the Token object
//         let Token = scenario.take_from_sender<Token>();
//         // Verify number of created swords
//         assert!(Token.sum == 0 && Token.actionsState == 0, 1);
//         // Return the Token object to the object pool
//         scenario.return_to_sender(Token);
//     };

//     // Third transaction executed by admin to create the Token
//     scenario.next_tx(admin);
//     {
//         let mut Token = scenario.take_from_sender<Token>();
//         // Create the add and transfer it to the initial owner
//         add(&mut Token, 10);
//         scenario.return_to_sender(Token);
//     };
//     scenario.end();
// }
