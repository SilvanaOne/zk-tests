module commitment::polynomial;

use std::debug;
use sui::bls12381::{
    Scalar,
    scalar_add,
    scalar_mul,
    scalar_from_bytes,
    scalar_zero,
    scalar_one
};
use sui::group_ops::Element;

/// should be random, less than 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001

fun get_s(): Element<Scalar> {
    let s_bytes =
        x"1582695da6689f26db7bb3eb32907ecd0ac3af032aefad31a069352705f0d459";
    scalar_from_bytes(&s_bytes)
}

fun get_r(): Element<Scalar> {
    let r_bytes =
        x"149fa8c209ab655fd480a3aff7d16dc72b6a3943e4b95fcf7909f42d9c17a552";
    scalar_from_bytes(&r_bytes)
}

// Using built-in scalar_zero() from sui::bls12381 instead

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
    let mut i = 0;
    let r = get_r();
    while (i < vector::length(&table)) {
        acc = scalar_add(&scalar_mul(&acc, &r), vector::borrow(&table, i));
        i = i + 1;
    };
    acc
}

/// constant-time single-field update
public fun update(
    old_c: &Element<Scalar>,
    delta: &Element<Scalar>, // new − old field value
    s_pow_j: &Element<Scalar>, // cached S^j
    r_pow_i: &Element<Scalar>, // cached R^i
): Element<Scalar> {
    let delta_struct = scalar_mul(delta, s_pow_j);
    let delta_table = scalar_mul(&delta_struct, r_pow_i);
    scalar_add(old_c, &delta_table)
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

    // update field 1 of struct 0: 2 -> 7  (Δ = 5)
    let delta = scalar_from_u256(5);

    // powers
    // field index 1 in a 2‑field struct has exponent 0 (S⁰ = 1)
    let s_pow_1 = scalar_one(); // S⁰
    let r_pow_0 = get_r(); // R1

    let commit1 = update(&commit0, &delta, &s_pow_1, &r_pow_0);

    // recompute ground‑truth commit
    let struct1_new = vector[scalar_from_u256(1), scalar_from_u256(7)];
    let c0_new = digest_struct(struct1_new);
    let table1 = vector[c0_new, c1];
    let commit_truth = commit(table1);

    assert!(commit1 == commit_truth, 0);
    debug::print(&b"commit1".to_ascii_string());
    debug::print(&commit1);
    debug::print(&b"commit_truth".to_ascii_string());
    debug::print(&commit_truth);
}
