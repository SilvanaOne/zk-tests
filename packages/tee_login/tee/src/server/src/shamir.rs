use anyhow::anyhow;
use bc_rand::SecureRandomNumberGenerator;
use bc_shamir::{recover_secret, split_secret};
use bip39::Mnemonic;
use zeroize::Zeroizing;

const THRESHOLD: usize = 10;
const SHARES: usize = 16; // bc‑shamir supports up to 16 shares

/// Split a 12‑word mnemonic into 16 Shamir shares.
pub fn split_mnemonic(mnemonic: &Mnemonic) -> anyhow::Result<Zeroizing<Vec<Vec<u8>>>> {
    let secret = Zeroizing::new(mnemonic.to_entropy());

    let mut rng = SecureRandomNumberGenerator;
    let shares = Zeroizing::new(split_secret(THRESHOLD, SHARES, &secret, &mut rng)?);

    Ok(shares)
}

/// Recover the original 12‑word mnemonic from ≥10 shares.
#[allow(dead_code, clippy::ptr_arg)]
pub fn recover_mnemonic(shares: &Vec<Vec<u8>>) -> anyhow::Result<String> {
    // Use sequential indices for the shares we have
    let indexes: Vec<usize> = (0..shares.len()).collect();
    let secret = Zeroizing::new(recover_secret(&indexes, shares)?);
    // secret is Vec<u8>; convert back to mnemonic
    let mnemonic = Zeroizing::new(Mnemonic::from_entropy(&secret).map_err(|e| anyhow!(e))?);
    Ok(mnemonic.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seed::generate_seed;

    #[test]
    fn test_split_and_recover_12_word_mnemonic() {
        // Generate a 12-word mnemonic
        let original_mnemonic = generate_seed(12);
        let original_string = original_mnemonic.to_string();

        // Split the mnemonic into shares
        let shares = split_mnemonic(&original_mnemonic).expect("Failed to split mnemonic");

        // Verify we have the expected number of shares
        assert_eq!(shares.len(), SHARES);

        // Take exactly the threshold number of shares
        let subset_shares = shares.iter().take(THRESHOLD).cloned().collect();

        // Recover the mnemonic from the threshold shares
        let recovered_string =
            recover_mnemonic(&subset_shares).expect("Failed to recover mnemonic");

        // Verify the recovered mnemonic matches the original
        assert_eq!(original_string, recovered_string);
    }

    #[test]
    fn test_split_and_recover_24_word_mnemonic() {
        // Generate a 24-word mnemonic
        let original_mnemonic = generate_seed(24);
        let original_string = original_mnemonic.to_string();

        // Split the mnemonic into shares
        let shares = split_mnemonic(&original_mnemonic).expect("Failed to split mnemonic");

        // Verify we have the expected number of shares
        assert_eq!(shares.len(), SHARES);

        // Take exactly the threshold number of shares
        let subset_shares: Vec<Vec<u8>> = shares.iter().take(THRESHOLD).cloned().collect();

        // Recover the mnemonic from the threshold shares
        let recovered_string =
            recover_mnemonic(&subset_shares).expect("Failed to recover mnemonic");

        // Verify the recovered mnemonic matches the original
        assert_eq!(original_string, recovered_string);
    }

    #[test]
    fn test_recover_with_more_than_threshold_shares() {
        // Generate a mnemonic
        let original_mnemonic = generate_seed(12);
        let original_string = original_mnemonic.to_string();

        // Split the mnemonic into shares
        let shares = split_mnemonic(&original_mnemonic).expect("Failed to split mnemonic");

        // Take more than the threshold number of shares (15 out of 20)
        let subset_shares: Vec<Vec<u8>> = shares.iter().take(15).cloned().collect();

        // Recover the mnemonic
        let recovered_string =
            recover_mnemonic(&subset_shares).expect("Failed to recover mnemonic");

        // Verify the recovered mnemonic matches the original
        assert_eq!(original_string, recovered_string);
    }

    #[test]
    fn test_recover_with_insufficient_shares_fails() {
        // Generate a mnemonic
        let original_mnemonic = generate_seed(12);

        // Split the mnemonic into shares
        let shares = split_mnemonic(&original_mnemonic).expect("Failed to split mnemonic");

        // Take fewer than the threshold number of shares (only 5 out of required 10)
        let subset_shares: Vec<Vec<u8>> = shares.iter().take(5).cloned().collect();

        // Attempt to recover the mnemonic should fail
        let result = recover_mnemonic(&subset_shares);
        assert!(
            result.is_err(),
            "Recovery should fail with insufficient shares"
        );
    }

    #[test]
    fn test_recover_with_exact_threshold_shares() {
        // Generate a mnemonic
        let original_mnemonic = generate_seed(12);
        let original_string = original_mnemonic.to_string();

        // Split the mnemonic into shares
        let shares = split_mnemonic(&original_mnemonic).expect("Failed to split mnemonic");

        // Take exactly the threshold number of shares (minimum required)
        let subset_shares: Vec<Vec<u8>> = shares.iter().take(THRESHOLD).cloned().collect();

        // Recover using the convenience function
        let recovered_string =
            recover_mnemonic(&subset_shares).expect("Failed to recover mnemonic");

        // Verify the recovered mnemonic matches the original
        assert_eq!(original_string, recovered_string);
    }

    #[test]
    fn test_multiple_split_recover_cycles() {
        // Test that splitting and recovering multiple times produces consistent results
        let original_mnemonic = generate_seed(12);
        let original_string = original_mnemonic.to_string();

        for _ in 0..5 {
            // Split the mnemonic
            let shares = split_mnemonic(&original_mnemonic).expect("Failed to split mnemonic");

            // Recover using threshold shares
            let subset_shares: Vec<Vec<u8>> = shares.iter().take(THRESHOLD).cloned().collect();
            let recovered_string =
                recover_mnemonic(&subset_shares).expect("Failed to recover mnemonic");

            // Verify consistency
            assert_eq!(original_string, recovered_string);
        }
    }

    #[test]
    fn test_shares_are_different() {
        // Generate a mnemonic
        let original_mnemonic = generate_seed(12);

        // Split the mnemonic into shares
        let shares = split_mnemonic(&original_mnemonic).expect("Failed to split mnemonic");

        // Verify that all shares are different from each other
        for i in 0..shares.len() {
            for j in (i + 1)..shares.len() {
                assert_ne!(
                    shares[i], shares[j],
                    "Shares {} and {} should be different",
                    i, j
                );
            }
        }

        // Verify that shares are different from the original entropy
        let original_entropy = original_mnemonic.to_entropy();
        for (i, share) in shares.iter().enumerate() {
            // Shares should be different from original entropy
            assert_ne!(
                &original_entropy, share,
                "Share {} should not equal original entropy",
                i
            );
        }
    }
}
