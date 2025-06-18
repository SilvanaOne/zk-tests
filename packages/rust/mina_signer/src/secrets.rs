// secrets.rs
#![forbid(unsafe_code)]

use once_cell::sync::Lazy;
use secrecy::{ExposeSecret, SecretVec};
use std::{collections::HashMap, sync::RwLock};

/// Every secret is an owned byte-vector wrapped in `secrecy`.
type SecretBytes = SecretVec<u8>;

/// The global, lazily-initialised secret map.
/// * `RwLock`         → many readers, one writer
/// * `HashMap<String>`→ string key (token id, username, etc.)
static SECRETS: Lazy<RwLock<HashMap<String, SecretBytes>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Insert or replace a secret.
///
/// `value` is moved straight into a `SecretVec`, so it never exists
/// in memory outside the secrecy wrapper.
pub fn add(key: &str, value: Vec<u8>) {
    SECRETS
        .write()
        .expect("lock poisoned")
        .insert(key.into(), SecretVec::new(value));
}

/// Execute `f` with a **borrow** of the secret bytes, if present.
///
/// The secret is decrypted/visible only while `f` runs; the read-lock
/// and borrow guard both go out of scope afterwards.
pub fn with_secret<R, F>(key: &str, f: F) -> Option<R>
where
    F: for<'a> FnOnce(&'a [u8]) -> R,
{
    let map = SECRETS.read().expect("lock poisoned");
    map.get(key).map(|sec| f(sec.expose_secret()))
}

/// Remove a secret, causing its bytes to be **zero-wiped** immediately.
pub fn remove(key: &str) -> bool {
    SECRETS
        .write()
        .expect("lock poisoned")
        .remove(key)
        .is_some()
}
