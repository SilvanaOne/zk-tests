//! Jellyfish tree verifiers.
//!
//! `stock` runs the unchanged `jmt` crate's update proof, with a guard for what it leaves unchecked.
//! `batch` is a deduplicated witness over the same tree, giving the same roots.

pub mod batch;
pub mod guard;
pub mod hashers;
pub mod stock;
