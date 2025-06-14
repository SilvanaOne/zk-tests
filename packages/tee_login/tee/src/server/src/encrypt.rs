use crate::db::Share;
use crate::logger::log_encryption_error;
use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use rsa::{Oaep, RsaPublicKey, pkcs8::DecodePublicKey, rand_core::OsRng};
use sha2::Sha512;
use zeroize::Zeroizing;

pub fn encrypt(data: &[u8], public_key: &str) -> Result<String> {
    let der = B64.decode(public_key).context("bad base64")?;
    let rsa_pub = RsaPublicKey::from_public_key_der(&der).context("not an RSA public key")?;

    let mut rng = OsRng;
    let padding = Oaep::new::<Sha512>();
    let cipher = rsa_pub
        .encrypt(&mut rng, padding, data)
        .context("encryption failed")?;

    Ok(B64.encode(cipher))
}

pub fn encrypt_shares(shares: &[Share], public_key: &str) -> Result<Vec<String>> {
    let mut encrypted_shares = Vec::new();
    for share in shares {
        let plaintext = Zeroizing::new(match bincode::serialize(&share) {
            Ok(data) => data,
            Err(e) => {
                let error_msg = format!("Failed to serialize shares: {}", e);
                log_encryption_error(&error_msg);
                return Err(anyhow::anyhow!(error_msg));
            }
        });

        let restored_data = match bincode::deserialize::<Share>(&plaintext) {
            Ok(data) => data,
            Err(e) => {
                let error_msg = format!("Failed to deserialize shares: {}", e);
                log_encryption_error(&error_msg);
                return Err(anyhow::anyhow!(error_msg));
            }
        };
        assert_eq!(restored_data.index, share.index);
        assert_eq!(restored_data.data, share.data);

        encrypted_shares.push(encrypt(&plaintext, public_key)?);
    }
    Ok(encrypted_shares)
}
