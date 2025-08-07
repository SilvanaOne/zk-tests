# IndexedMerkleMap

A Rust implementation of an Indexed Merkle Map, compatible with o1js IndexedMerkleMap and optimized for use in SP1 zkVM.

## Features

- **Efficient key-value storage**: Uses an indexed merkle tree structure for efficient membership and non-membership proofs
- **SHA256 hashing**: Uses SHA256 as the hash function, with SP1 precompile support when running in zkVM
- **Small tree height**: Configurable tree height (1-32) to minimize proof sizes and hashing operations
- **Membership proofs**: Generate and verify proofs that a key-value pair exists in the map
- **Non-membership proofs**: Generate and verify proofs that a key does not exist in the map
- **no_std support**: Can be used in embedded and zkVM environments

## Usage

```rust
use indexed_merkle_map::{IndexedMerkleMap, Field};

// Create a new map with height 10 (supports up to 2^10 - 1 = 1023 entries)
let mut map = IndexedMerkleMap::new(10);

// Insert key-value pairs
let key = Field::from_u32(100);
let value = Field::from_u32(200);
map.insert(key, value).unwrap();

// Update an existing value
let new_value = Field::from_u32(300);
let old_value = map.update(key, new_value).unwrap();

// Get membership proof
let proof = map.get_membership_proof(&key).unwrap();
let root = map.root();

// Verify the proof
assert!(IndexedMerkleMap::verify_membership_proof(&root, &proof, &key, &new_value));

// Get non-membership proof for a non-existent key
let non_existent_key = Field::from_u32(999);
let non_proof = map.get_non_membership_proof(&non_existent_key).unwrap();
assert!(IndexedMerkleMap::verify_non_membership_proof(&root, &non_proof, &non_existent_key));
```

## SP1 zkVM Support

This library is optimized for use in SP1 zkVM with:
- Patched `crypto-bigint` and `sha2` dependencies for better performance
- Support for SP1's SHA256 precompile when running in zkVM environment
- no_std support for zkVM execution

## Implementation Details

The IndexedMerkleMap uses a sorted linked-list structure within a Merkle tree:
- Each leaf contains: `(key, value, next_key)`
- Leaves are sorted by key with the linked list encoded via `next_key`
- The first leaf is always `(0, 0, 0)`
- Non-membership is proven by showing a "low leaf" where `low.key < target_key < low.next_key`

This design allows for:
- Efficient non-membership proofs without sparse Merkle trees
- Small tree heights (e.g., height 20 for ~1 million entries)
- Significantly fewer hash operations compared to sparse 256-level trees

## License

MIT