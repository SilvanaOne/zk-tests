//! Verifiers for an indexed Merkle map and a Jellyfish tree, generic over the hash.
//!
//! Every engine is a pure function from witness bytes to public values. A guest wraps one with its
//! input and output calls, and the host runs the same function with software hashes, so the host
//! knows the exact precompile calls and public values a guest must produce.

pub mod hash;
pub mod imm;
pub mod jellyfish;
pub mod record;
