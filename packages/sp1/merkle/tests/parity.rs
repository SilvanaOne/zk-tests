//! The adapted map must build exactly the trees the original crate builds with SHA-256: the same
//! root, length, lookups and errors after every operation.

use indexed_merkle_map::{Field, IndexedMerkleMap as Original};
use merkle_bench::software::Native;
use merkle_map::{
    hash::{Hasher, Sha256Raw},
    imm::host::IndexedMerkleMap,
};

type Sha = Sha256Raw<Native>;

struct Pair {
    original: Original,
    adapted: IndexedMerkleMap<Sha>,
}

impl Pair {
    fn new(height: usize) -> Self {
        let pair = Self {
            original: Original::new(height),
            adapted: IndexedMerkleMap::new(height),
        };
        pair.check_state("new");
        pair
    }

    fn check_state(&self, step: &str) {
        assert_eq!(
            self.original.root().to_bytes(),
            Sha::export(&self.adapted.root()),
            "root after {step}"
        );
        assert_eq!(
            self.original.length() as u64,
            self.adapted.length(),
            "length after {step}"
        );
    }

    fn check_lookup(&self, key: &[u8; 32], step: &str) {
        assert_eq!(
            self.original
                .get_option(&Field::from_bytes(*key))
                .map(|value| value.to_bytes()),
            self.adapted
                .get_option(&Sha::import(key))
                .map(|value| Sha::export(&value)),
            "lookup after {step}"
        );
    }

    fn insert(&mut self, key: [u8; 32], value: [u8; 32]) {
        let original = self
            .original
            .insert(Field::from_bytes(key), Field::from_bytes(value))
            .map_err(|error| format!("{error:?}"));
        let adapted = self
            .adapted
            .insert(Sha::import(&key), Sha::import(&value))
            .map_err(|error| format!("{error:?}"));
        assert_eq!(original, adapted, "insert of {key:?}");
        self.check_state("insert");
        self.check_lookup(&key, "insert");
    }

    fn update(&mut self, key: [u8; 32], value: [u8; 32]) {
        let original = self
            .original
            .update(Field::from_bytes(key), Field::from_bytes(value))
            .map(|old| old.to_bytes())
            .map_err(|error| format!("{error:?}"));
        let adapted = self
            .adapted
            .update(Sha::import(&key), Sha::import(&value))
            .map(|old| Sha::export(&old))
            .map_err(|error| format!("{error:?}"));
        assert_eq!(original, adapted, "update of {key:?}");
        self.check_state("update");
        self.check_lookup(&key, "update");
    }

    fn set(&mut self, key: [u8; 32], value: [u8; 32]) {
        let original = self
            .original
            .set(Field::from_bytes(key), Field::from_bytes(value))
            .map(|old| old.map(|old| old.to_bytes()))
            .map_err(|error| format!("{error:?}"));
        let adapted = self
            .adapted
            .set(Sha::import(&key), Sha::import(&value))
            .map(|old| old.map(|old| Sha::export(&old)))
            .map_err(|error| format!("{error:?}"));
        assert_eq!(original, adapted, "set of {key:?}");
        self.check_state("set");
        self.check_lookup(&key, "set");
    }
}

fn small(value: u8) -> [u8; 32] {
    let mut key = [0u8; 32];
    key[31] = value;
    key
}

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut x = *state;
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

fn random_bytes(state: &mut u64) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    for chunk in bytes.chunks_mut(8) {
        chunk.copy_from_slice(&splitmix(state).to_le_bytes());
    }
    bytes
}

#[test]
fn edge_keys_match_the_original() {
    for height in [2, 3, 4, 8] {
        let mut pair = Pair::new(height);
        let (max, top) = ([0xff; 32], {
            let mut key = [0u8; 32];
            key[0] = 0x80;
            key
        });
        // The zero key is taken by the sentinel, and can still be updated, as in the original.
        pair.insert(small(0), small(9));
        pair.update(small(0), small(10));
        for key in [
            max,
            small(1),
            top,
            small(2),
            small(1),
            max,
            small(3),
            small(255),
        ] {
            pair.insert(key, small(7));
        }
        pair.update(small(200), small(1));
        pair.update(small(1), small(8));
        pair.update(max, small(8));
        pair.set(small(1), small(11));
        pair.set(small(4), small(12));
    }
}

#[test]
fn reversed_and_sorted_orders_match_the_original() {
    for keys in [
        (1..=40).collect::<Vec<u8>>(),
        (1..=40).rev().collect::<Vec<u8>>(),
    ] {
        let mut pair = Pair::new(8);
        for key in keys {
            pair.insert(small(key), small(key));
        }
    }
}

#[test]
fn random_operations_match_the_original() {
    for height in [2, 3, 4, 8, 16, 20] {
        let mut state = height as u64 * 1000;
        let mut pair = Pair::new(height);
        let mut known: Vec<[u8; 32]> = Vec::new();
        for _ in 0..400 {
            let value = random_bytes(&mut state);
            let reuse = !known.is_empty() && splitmix(&mut state).is_multiple_of(2);
            let key = if reuse {
                known[splitmix(&mut state) as usize % known.len()]
            } else {
                let key = random_bytes(&mut state);
                known.push(key);
                key
            };
            match splitmix(&mut state) % 3 {
                0 => pair.insert(key, value),
                1 => pair.update(key, value),
                _ => pair.set(key, value),
            }
        }
    }
}
