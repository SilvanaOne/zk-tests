//! The indexed Merkle map, adapted from `crates/indexed-merkle-map` in
//! <https://github.com/SilvanaOne/silvana> (Apache-2.0).
//!
//! Changes from the original:
//! - generic over the hash through `ImmHasher`, with digests in the hash's native words
//! - an insert binds the new leaf's path to the updated low-leaf root and proves the slot is empty
//! - proofs have exactly the tree's depth, and positions come from the leaf index
//! - witnesses drop what a guest can derive, and a guest computes its own roots
//! - the host map has no linear scans, and can be bulk loaded
//! - `batch` is new: one deduplicated witness for a whole batch

pub mod batch;
pub mod chained;
pub mod host;
pub mod provable;
pub mod types;
