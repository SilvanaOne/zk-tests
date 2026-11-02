//! Checks `jmt`'s update proof leaves to its caller.
//!
//! `verify_update` applies a write whether the key is new or not, so an insert over an existing key
//! would pass as an overwrite. It also computes a split from a supplied leaf without first checking
//! that the new key belongs under that leaf. These checks make adding and changing a record prove
//! what they claim. They read the Borsh bytes because the fields they need are private. A proof is
//! laid out as: a leaf tag, an optional 64-byte leaf (key hash, value hash), and a sibling count.

const LEAF_TAG_NONE: u8 = 0;
const LEAF_TAG_SOME: u8 = 1;
const MAX_SIBLINGS: u32 = 256;

/// A leaf's key hash and value hash.
type ProofLeaf<'a> = (&'a [u8; 32], &'a [u8; 32]);

/// The leaf a proof carries, and how many siblings it has.
fn parse(frame: &[u8]) -> (Option<ProofLeaf<'_>>, u32) {
    match frame[0] {
        LEAF_TAG_NONE => (None, count_at(frame, 1)),
        LEAF_TAG_SOME => {
            let key = frame[1..33].try_into().unwrap();
            let value = frame[33..65].try_into().unwrap();
            (Some((key, value)), count_at(frame, 65))
        }
        tag => panic!("bad leaf tag {tag}"),
    }
}

fn count_at(frame: &[u8], offset: usize) -> u32 {
    let count = u32::from_le_bytes(frame[offset..offset + 4].try_into().unwrap());
    assert!(count <= MAX_SIBLINGS, "too many siblings");
    count
}

/// The number of leading bits two hashes share, in the tree's path order.
fn common_prefix_bits(a: &[u8; 32], b: &[u8; 32]) -> u32 {
    for (index, (x, y)) in a.iter().zip(b).enumerate() {
        if x != y {
            return index as u32 * 8 + (x ^ y).leading_zeros();
        }
    }
    256
}

/// The key must be absent: either the path ends in an empty slot, or in a leaf for another key whose
/// subtree the key falls into. A leaf with the same key, or one the key does not fall under, fails.
#[inline(always)]
pub fn check_insert(frame: &[u8], key: &[u8; 32]) {
    let (leaf, siblings) = parse(frame);
    if let Some((leaf_key, _)) = leaf {
        assert!(leaf_key != key, "insert of an existing key");
        assert!(
            common_prefix_bits(leaf_key, key) >= siblings,
            "the key does not belong under this leaf"
        );
    }
}

/// The key must be present with the old value the update builds on.
#[inline(always)]
pub fn check_update(frame: &[u8], key: &[u8; 32], old_value_hash: &[u8; 32]) {
    let (leaf, _) = parse(frame);
    let (leaf_key, leaf_value) = leaf.expect("update of an absent key");
    assert!(leaf_key == key, "update through another key's leaf");
    assert!(leaf_value == old_value_hash, "wrong old value");
}
