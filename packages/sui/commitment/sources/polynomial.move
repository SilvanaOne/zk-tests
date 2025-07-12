module commitment::polynomial;

use std::debug;
use sui::bls12381::{
    Scalar,
    scalar_add,
    scalar_sub,
    scalar_mul,
    scalar_from_bytes,
    scalar_zero,
    scalar_from_u64
};
use sui::group_ops::Element;

#[test_only]
use sui::test_scenario as test;
#[test_only]
use sui::random;

/// should be random, less than 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001
fun get_s(): Element<Scalar> {
    let s_bytes =
        x"1582695da6689f26db7bb3eb32907ecd0ac3af032aefad31a069352705f0d459";
    let s = scalar_from_bytes(&s_bytes);
    s
}

fun get_r(): Element<Scalar> {
    let r_bytes =
        x"149fa8c209ab655fd480a3aff7d16dc72b6a3943e4b95fcf7909f42d9c17a552";
    scalar_from_bytes(&r_bytes)
}

/// inner: digest one struct
public fun digest_struct(fields: vector<Element<Scalar>>): Element<Scalar> {
    let mut d = scalar_zero();
    let mut i = 0;
    let s = get_s();
    while (i < vector::length(&fields)) {
        d = scalar_add(&scalar_mul(&d, &s), vector::borrow(&fields, i));
        i = i + 1;
    };
    d
}

/// outer: commit whole table
public fun commit(table: vector<Element<Scalar>>): Element<Scalar> {
    // table[i] already holds c_i = digest_struct(...)
    let mut acc = scalar_zero();
    let r = get_r();
    let len = vector::length(&table);
    let mut i = len;

    // Iterate in reverse order so that table[i] gets coefficient r^i
    while (i > 0) {
        i = i - 1;
        acc = scalar_add(&scalar_mul(&acc, &r), vector::borrow(&table, i));
    };
    acc
}

/// Helper function to compute r^exp
fun scalar_pow(base: &Element<Scalar>, exp: u64): Element<Scalar> {
    let mut acc = scalar_from_u64(1); // Start with 1
    let mut i = 0;
    while (i < exp) {
        acc = scalar_mul(&acc, base);
        i = i + 1;
    };
    acc
}

/// constant-time single-field update using struct digest recalculation
public fun update(
    old_table_commitment: &Element<Scalar>,
    old_struct_digest: &Element<Scalar>, // old struct digest at position i
    new_struct_digest: &Element<Scalar>, // new struct digest at position i
    index: u64, // index in table (0-based)
): Element<Scalar> {
    // The table commitment formula in commit() now produces:
    // table[0]*r^0 + table[1]*r^1 + table[2]*r^2 + ... + table[i]*r^i
    // So position i has coefficient r^i

    let r = get_r();
    // Position i has coefficient r^i
    let r_pow_i = scalar_pow(&r, index);

    let struct_delta = scalar_sub(new_struct_digest, old_struct_digest);
    let table_delta = scalar_mul(&struct_delta, &r_pow_i);
    scalar_add(old_table_commitment, &table_delta)
}

/// Mina (Pallas) field modulus  p = 0x4000...0001
const MINA_PRIME: u256 =
    0x40000000000000000000000000000000224698FC094CF91B992D30ED00000001;

/// Convert a u256 into a BLS scalar *iff* it is < MINA_PRIME
public fun scalar_from_u256(n: u256): Element<Scalar> {
    // abort 1 if out of range
    assert!(n < MINA_PRIME, 1);

    // 32‑byte big‑endian buffer
    let mut bytes =
        x"0000000000000000000000000000000000000000000000000000000000000000";

    let mut tmp = n;
    let mut i: u8 = 31;
    loop {
        // write least‑significant byte of tmp into bytes[i]
        *vector::borrow_mut(&mut bytes, i as u64) = (tmp & 0xff) as u8;
        if (i == 0) break;
        tmp = tmp >> 8;
        i = i - 1;
    };

    scalar_from_bytes(&bytes)
}

// #[allow(unused_function)]
// fun scalar_pow(base: &Element<Scalar>, exp: u8): Element<Scalar> {
//     let mut acc = one_scalar();
//     let mut i = 0;
//     while (i < exp) {
//         acc = scalar_mul(&acc, base);
//         i = i + 1;
//     };
//     acc
// }

#[test]
fun test_commit_update() {
    // two structs, each 2 fields
    let struct1 = vector[
        scalar_from_u256(1), // field 0 = 1
        scalar_from_u256(2), // field 1 = 2
    ];
    let struct2 = vector[
        scalar_from_u256(3), // field 0 = 3
        scalar_from_u256(4), // field 1 = 4
    ];

    let c0 = digest_struct(struct1);
    let c1 = digest_struct(struct2);
    let table0 = vector[c0, c1];
    let commit0 = commit(table0);
    debug::print(&b"commit0".to_ascii_string());
    debug::print(&commit0);

    // update field 1 of struct 0: 2 -> 7
    // Create the new struct with updated field
    let struct1_new = vector[scalar_from_u256(1), scalar_from_u256(7)];

    // Calculate old and new struct digests
    let old_struct_digest = c0; // We already computed this as digest_struct(struct1)
    let new_struct_digest = digest_struct(struct1_new);

    // Update struct at index 0
    // For a 2-element table, position 0 has coefficient r^(2-1-0) = r^1 = r
    let commit1 = update(
        &commit0,
        &old_struct_digest,
        &new_struct_digest,
        0,
    );

    // recompute ground‑truth commit
    let c0_new = new_struct_digest; // We already computed this above
    let table1 = vector[c0_new, c1];
    let commit_truth = commit(table1);

    assert!(commit1 == commit_truth, 0);
    debug::print(&b"commit1".to_ascii_string());
    debug::print(&commit1);
}

#[test]
fun test_large_table_random_updates() {
    let alice: address = @0xa11ce;
    let scenario = test::begin(alice);

    // Create random generator
    let mut rng = random::new_generator_for_testing();

    // Create 10 structs with random initial values
    let mut structs: vector<vector<Element<Scalar>>> = vector[];
    let mut i = 0;
    while (i < 10) {
        let field0_val = random::generate_u64_in_range(&mut rng, 1, 1000);
        let field1_val = random::generate_u64_in_range(&mut rng, 1, 1000);
        let struct_fields = vector[
            scalar_from_u256(field0_val as u256),
            scalar_from_u256(field1_val as u256),
        ];
        vector::push_back(&mut structs, struct_fields);
        i = i + 1;
    };

    // Compute initial struct digests and table commitment
    let mut table_digests: vector<Element<Scalar>> = vector[];
    i = 0;
    while (i < 10) {
        let digest = digest_struct(*vector::borrow(&structs, i));
        vector::push_back(&mut table_digests, digest);
        i = i + 1;
    };

    let mut current_commitment = commit(table_digests);

    // Generate random update sequence
    let mut update_indices = vector[
        0,
        1,
        2,
        3,
        4,
        5,
        6,
        7,
        8,
        9,
        0,
        1,
        2,
        3,
        4,
    ]; // Some repeated indices
    random::shuffle(&mut rng, &mut update_indices);

    let num_updates = vector::length(&update_indices);
    let mut update_count = 0;
    while (update_count < num_updates) {
        let index = *vector::borrow(&update_indices, update_count);

        // Generate random new values
        let new_field0 = random::generate_u64_in_range(&mut rng, 1, 2000);
        let new_field1 = random::generate_u64_in_range(&mut rng, 1, 2000);

        // Get old struct digest
        let old_struct_digest = *vector::borrow(&table_digests, index);

        // Create new struct and compute its digest
        let new_struct = vector[
            scalar_from_u256(new_field0 as u256),
            scalar_from_u256(new_field1 as u256),
        ];
        let new_struct_digest = digest_struct(new_struct);

        // Update using incremental function
        current_commitment =
            update(
                &current_commitment,
                &old_struct_digest,
                &new_struct_digest,
                index,
            );

        // Update our tracking table
        *vector::borrow_mut(&mut table_digests, index) = new_struct_digest;
        *vector::borrow_mut(&mut structs, index) = new_struct;

        // Recompute from scratch for verification
        let mut verification_table: vector<Element<Scalar>> = vector[];
        let mut j = 0;
        while (j < 10) {
            let digest = digest_struct(*vector::borrow(&structs, j));
            vector::push_back(&mut verification_table, digest);
            j = j + 1;
        };
        let truth_commitment = commit(verification_table);

        // Verify they match
        assert!(current_commitment == truth_commitment, update_count);

        update_count = update_count + 1;
    };

    test::end(scenario);
}
