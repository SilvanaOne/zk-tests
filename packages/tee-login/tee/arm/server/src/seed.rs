use bip39::{Language, Mnemonic};

pub fn generate_seed(words: usize) -> Mnemonic {
    // Accept only standard BIP‑39 word counts
    assert!(words == 12 || words == 24, "only 12 or 24 words allowed");

    Mnemonic::generate_in(Language::English, words).expect("failed to generate mnemonic")
}
