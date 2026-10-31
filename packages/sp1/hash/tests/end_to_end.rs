//! Executes every guest without proving. Each run must match the native hash and make exactly the
//! expected precompile calls, which shows the host Poseidon2 implementation and the raw SHA-256 and
//! Keccak-256 guests are correct and that the precompile patches are active.

use hash_bench::{bench::execute_and_check, data::Dataset, variant::Variant};
use sp1_sdk::LightProver;

/// Sizes on either side of each hash's block and padding boundaries.
const SIZES: [usize; 16] = [
    1, 23, 24, 25, 40, 55, 56, 63, 64, 65, 96, 135, 136, 137, 272, 1024,
];

#[tokio::test(flavor = "multi_thread")]
async fn guests_match_native_hashes_and_expected_precompile_calls() {
    let light = LightProver::new().await;
    for size in SIZES {
        let data = Dataset::generate(42, size, 3);
        for variant in Variant::ALL.into_iter().filter(|v| v.runs_at(size)) {
            let result = execute_and_check(&light, variant, &data)
                .await
                .unwrap_or_else(|error| panic!("{} at {size} bytes: {error:#}", variant.name()));
            assert!(result.cycles > 0);
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_digest_mismatch_is_caught() {
    let light = LightProver::new().await;
    let mut data = Dataset::generate(1, 64, 2);
    // The guest reads `raw`, while the native side hashes `swapped`, so changing it breaks the match.
    data.swapped[0] ^= 1;
    let error = execute_and_check(&light, Variant::Sha256Node, &data)
        .await
        .expect_err("a digest mismatch must fail");
    assert!(
        error.to_string().contains("native hash"),
        "unexpected error: {error}"
    );
}
