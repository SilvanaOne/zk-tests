//! The hashes on the host, using each crate's default fastest path, and their guest output format.
//!
//! `accumulate` mirrors each guest exactly, so comparing its result with a guest's public values
//! proves the guest computed the real hash of every set.

use std::sync::LazyLock;

use sha2::{Digest, Sha256};
use sha3::Keccak256;
use slop_algebra::{AbstractField, PrimeField32};
use slop_symmetric::Permutation;
use sp1_primitives::{SP1Field, SP1Perm, poseidon2_init};

use crate::{
    data::Dataset,
    variant::{NODE_SIZE, Variant},
};

/// The digest accumulator, as the guests commit it.
pub type Lanes = [u64; 4];

/// The bytes one Poseidon2 absorb takes: 8 rate elements of 3 bytes each.
const BYTE_BLOCK_SIZE: usize = 24;

/// The permutation behind SP1's precompile: KoalaBear, width 16, 8 full and 20 partial rounds.
static PERMUTATION: LazyLock<SP1Perm> = LazyLock::new(poseidon2_init);

pub fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

pub fn keccak256(data: &[u8]) -> [u8; 32] {
    Keccak256::digest(data).into()
}

/// SP1's `Poseidon2ByteHash`, which only runs inside the zkVM: a length block, then the data as
/// zero-padded 24-byte blocks, overwriting the rate before each permutation.
pub fn poseidon2_bytes(permutation: &SP1Perm, input: &[u8]) -> [u32; 8] {
    let mut state = [SP1Field::zero(); 16];
    let mut length_block = [0u8; BYTE_BLOCK_SIZE];
    length_block[..8].copy_from_slice(&(input.len() as u64).to_le_bytes());
    absorb(permutation, &mut state, &length_block);

    let (blocks, remainder) = input.as_chunks::<BYTE_BLOCK_SIZE>();
    for block in blocks {
        absorb(permutation, &mut state, block);
    }
    if !remainder.is_empty() {
        let mut last_block = [0u8; BYTE_BLOCK_SIZE];
        last_block[..remainder.len()].copy_from_slice(remainder);
        absorb(permutation, &mut state, &last_block);
    }
    core::array::from_fn(|i| state[i].as_canonical_u32())
}

fn absorb(permutation: &SP1Perm, state: &mut [SP1Field; 16], block: &[u8; BYTE_BLOCK_SIZE]) {
    for (element, bytes) in state.iter_mut().zip(block.as_chunks::<3>().0) {
        *element =
            SP1Field::from_canonical_u32(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]));
    }
    permutation.permute_mut(state);
}

/// `Trunc8(P(x))` of 16 canonical elements.
pub fn poseidon2_node(permutation: &SP1Perm, node: &[u32; 16]) -> [u32; 8] {
    let output = permutation.permute(node.map(SP1Field::from_canonical_u32));
    core::array::from_fn(|i| output[i].as_canonical_u32())
}

/// `Trunc8(P(x) + x)` of 16 canonical elements.
pub fn poseidon2_node_ff(permutation: &SP1Perm, node: &[u32; 16]) -> [u32; 8] {
    let input = node.map(SP1Field::from_canonical_u32);
    let output = permutation.permute(input);
    core::array::from_fn(|i| (output[i] + input[i]).as_canonical_u32())
}

/// Hashes every set once and returns the XOR of the digests in the guest's format.
pub fn accumulate(variant: Variant, data: &Dataset) -> Lanes {
    match variant {
        Variant::Baseline => baseline(data),
        Variant::Sha256 => fold(data.sets(), |set| byte_lanes(&sha256(set))),
        Variant::Keccak256 | Variant::Keccak256Node => {
            fold(data.sets(), |set| byte_lanes(&keccak256(set)))
        }
        Variant::Poseidon2 => {
            let permutation = &*PERMUTATION;
            fold(data.sets(), |set| {
                element_lanes(&poseidon2_bytes(permutation, set))
            })
        }
        Variant::Sha256Node => fold(nodes(&data.swapped), |message| word_lanes(&sha256(message))),
        Variant::Poseidon2Node => {
            let permutation = &*PERMUTATION;
            fold(nodes(&data.elements), |node| {
                element_lanes(&poseidon2_node(permutation, &node_elements(node)))
            })
        }
        Variant::Poseidon2NodeFf => {
            let permutation = &*PERMUTATION;
            fold(nodes(&data.elements), |node| {
                element_lanes(&poseidon2_node_ff(permutation, &node_elements(node)))
            })
        }
    }
}

fn nodes(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    bytes
        .as_chunks::<NODE_SIZE>()
        .0
        .iter()
        .map(|node| node.as_slice())
}

fn fold<'a>(sets: impl Iterator<Item = &'a [u8]>, mut hash: impl FnMut(&[u8]) -> Lanes) -> Lanes {
    let mut acc = [0u64; 4];
    for set in sets {
        for (a, lane) in acc.iter_mut().zip(hash(set)) {
            *a ^= lane;
        }
    }
    acc
}

/// XORs every word of every set, padding included, into the lanes in turn.
fn baseline(data: &Dataset) -> Lanes {
    let mut acc = [0u64; 4];
    for set in data.raw.chunks_exact(data.stride) {
        for (i, word) in set.as_chunks::<8>().0.iter().enumerate() {
            acc[i % 4] ^= u64::from_le_bytes(*word);
        }
    }
    acc
}

/// Lanes of a byte digest, read little-endian as a Keccak state or a byte buffer would be.
fn byte_lanes(digest: &[u8; 32]) -> Lanes {
    let (words, _) = digest.as_chunks::<8>();
    core::array::from_fn(|i| u64::from_le_bytes(words[i]))
}

/// Lanes of a SHA-256 digest held as its 8 state words.
fn word_lanes(digest: &[u8; 32]) -> Lanes {
    let (words, _) = digest.as_chunks::<4>();
    element_lanes(&core::array::from_fn(|i| u32::from_be_bytes(words[i])))
}

/// Lanes of 8 words, low word first.
fn element_lanes(words: &[u32; 8]) -> Lanes {
    core::array::from_fn(|i| words[2 * i] as u64 | (words[2 * i + 1] as u64) << 32)
}

fn node_elements(node: &[u8]) -> [u32; 16] {
    let (words, _) = node.as_chunks::<4>();
    core::array::from_fn(|i| u32::from_le_bytes(words[i]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn sha256_known_answer() {
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn keccak256_known_answer() {
        assert_eq!(
            hex(&keccak256(b"abc")),
            "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"
        );
    }

    /// The vector from SP1's own `TestPoseidonKoalaBear2`, which shows this is the permutation
    /// behind the precompile rather than another KoalaBear instance.
    #[test]
    fn poseidon2_matches_sp1s_permutation_vector() {
        let zeros = [SP1Field::zero(); 16];
        let output = PERMUTATION
            .permute(zeros)
            .map(|element| element.as_canonical_u32());
        assert_eq!(
            output,
            [
                145589356, 1876041682, 1734203622, 499355069, 673349476, 595701365, 270340205,
                131707822, 1236787881, 1085405948, 2065733208, 1999012278, 2062318124, 1616707536,
                324813015, 749520722,
            ]
        );
    }

    #[test]
    fn poseidon2_byte_hash_absorbs_a_length_block_first() {
        // With no data, the only absorb is an all-zero length block.
        assert_eq!(
            poseidon2_bytes(&PERMUTATION, b""),
            [
                145589356, 1876041682, 1734203622, 499355069, 673349476, 595701365, 270340205,
                131707822
            ]
        );
        // Trailing zeros change the length block, so they change the digest.
        assert_ne!(
            poseidon2_bytes(&PERMUTATION, b"abc"),
            poseidon2_bytes(&PERMUTATION, b"abc\0")
        );
    }

    #[test]
    fn poseidon2_feed_forward_adds_the_input() {
        let node: [u32; 16] = core::array::from_fn(|i| i as u32 + 1);
        let plain = poseidon2_node(&PERMUTATION, &node);
        let forwarded = poseidon2_node_ff(&PERMUTATION, &node);
        for i in 0..8 {
            let sum = (plain[i] as u64 + node[i] as u64) % crate::data::KOALA_BEAR_PRIME as u64;
            assert_eq!(forwarded[i] as u64, sum);
        }
    }

    #[test]
    fn baseline_xors_every_word_into_its_lane() {
        let data = Dataset::generate(9, 40, 2);
        let mut expected = [0u64; 4];
        for set in data.raw.as_chunks::<40>().0 {
            for (i, word) in set.as_chunks::<8>().0.iter().enumerate() {
                expected[i % 4] ^= u64::from_le_bytes(*word);
            }
        }
        assert_eq!(accumulate(Variant::Baseline, &data), expected);
    }

    #[test]
    fn node_variants_hash_the_same_message_as_the_byte_tier() {
        // sha256_node hashes the word-swapped set; its digest must be plain SHA-256 of that.
        let data = Dataset::generate(10, 64, 1);
        let digest = sha256(&data.swapped);
        assert_eq!(accumulate(Variant::Sha256Node, &data), word_lanes(&digest));
        // keccak256_node hashes the same bytes as keccak256.
        assert_eq!(
            accumulate(Variant::Keccak256Node, &data),
            accumulate(Variant::Keccak256, &data)
        );
    }
}
