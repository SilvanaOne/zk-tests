use crate::db::Share;
use crate::logger::log_encryption_error;
use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
// CMS imports removed since we're using manual parsing for AWS KMS indefinite length encoding
use rsa::{Oaep, RsaPrivateKey, RsaPublicKey, pkcs8::DecodePublicKey, rand_core::OsRng};
use sha2::{Sha256, Sha512};
use zeroize::Zeroizing;

#[derive(Debug, Clone)]
pub struct KMSKeyPair {
    pub public_key: RsaPublicKey,
    pub private_key: RsaPrivateKey,
}

pub fn generate_kms_key_pair() -> Result<KMSKeyPair> {
    let mut rng = OsRng;
    let private_key = RsaPrivateKey::new(&mut rng, 2048)?;
    let public_key = RsaPublicKey::from(&private_key);
    Ok(KMSKeyPair {
        public_key,
        private_key,
    })
}

pub fn decrypt(data: &[u8], private_key: &RsaPrivateKey) -> Result<Vec<u8>> {
    let padding = Oaep::new::<Sha256>();
    let plaintext = private_key
        .decrypt(padding, data)
        .context("KMS decryption failed")?;
    Ok(plaintext)
}

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

/// Decrypt PKCS#7 EnvelopedData from AWS KMS CiphertextForRecipient
/// This returns the original data‑key that `GenerateDataKey` produced.
pub fn decrypt_kms_ciphertext(data: &[u8], private_key: &RsaPrivateKey) -> Result<Vec<u8>> {
    use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
    // Concrete CBC decryptors for the three AES key sizes
    type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
    type Aes192CbcDec = cbc::Decryptor<aes::Aes192>;
    type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

    //
    // ─────────────────────────── Stage 1 ───────────────────────────
    // Locate the RSA‑encrypted content‑encryption key (CEK) inside the
    // RecipientInfo structure and decrypt it with RSA‑OAEP‑SHA‑256.
    //
    let recipient_start = data
        .iter()
        .position(|&b| b == 0x31) // SET
        .ok_or_else(|| anyhow::anyhow!("RecipientInfo SET not found"))?;

    // We expect a 256‑byte OCTET STRING: 04 82 01 00 <…256 bytes…>
    let rsa_key_offset = (recipient_start..data.len() - 4)
        .find(|&i| {
            data[i] == 0x04 && data[i + 1] == 0x82 && data[i + 2] == 0x01 && data[i + 3] == 0x00
        })
        .ok_or_else(|| anyhow::anyhow!("Encrypted CEK not found"))?;

    let encrypted_cek = &data[rsa_key_offset + 4..rsa_key_offset + 4 + 256];

    let cek = private_key
        .decrypt(Oaep::new::<Sha256>(), encrypted_cek)
        .context("RSA‑OAEP decryption of CEK failed")?;

    //
    // ─────────────────────────── Stage 2 ───────────────────────────
    // Find the AES‑256‑CBC algorithm identifier to obtain
    //   * the IV  (OCTET STRING of 16 bytes)
    //   * the ciphertext that is encrypted with the CEK
    //
    const AES256_CBC_OID: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x01, 0x2a];
    let oid_pos = data
        .windows(AES256_CBC_OID.len())
        .position(|w| w == AES256_CBC_OID)
        .ok_or_else(|| anyhow::anyhow!("AES‑256‑CBC OID not found"))?;

    // Parameter: 04 10 <IV>
    let iv_pos = oid_pos + AES256_CBC_OID.len();
    if data.get(iv_pos) != Some(&0x04) || data.get(iv_pos + 1) != Some(&0x10) {
        return Err(anyhow::anyhow!("Unexpected IV format"));
    }
    let iv = &data[iv_pos + 2..iv_pos + 18];

    // The encrypted content OCTET STRING begins right after the IV.
    // There is an optional context‑specific wrapper: a0 80
    let mut enc_pos = iv_pos + 18;
    if data.get(enc_pos) == Some(&0xa0) && data.get(enc_pos + 1) == Some(&0x80) {
        enc_pos += 2; // skip the wrapper
    }
    if data.get(enc_pos) != Some(&0x04) {
        return Err(anyhow::anyhow!("Encrypted content OCTET STRING not found"));
    }

    let len_byte = data[enc_pos + 1];
    let ciphertext = if len_byte == 0x80 {
        // Indefinite length – read until 00 00
        let mut end = enc_pos + 2;
        while !(data[end] == 0 && data[end + 1] == 0) {
            end += 1;
        }
        &data[enc_pos + 2..end]
    } else {
        let len = len_byte as usize;
        &data[enc_pos + 2..enc_pos + 2 + len]
    };

    //
    // ─────────────────────────── Stage 3 ───────────────────────────
    // AES‑CBC‑PKCS#7 decrypt
    //
    let mut buf = ciphertext.to_vec();
    let plaintext = match cek.len() {
        16 => Aes128CbcDec::new_from_slices(&cek, iv)
            .unwrap()
            .decrypt_padded_mut::<Pkcs7>(&mut buf)
            .map(|pt| pt.to_vec())?,
        24 => Aes192CbcDec::new_from_slices(&cek, iv)
            .unwrap()
            .decrypt_padded_mut::<Pkcs7>(&mut buf)
            .map(|pt| pt.to_vec())?,
        32 => Aes256CbcDec::new_from_slices(&cek, iv)
            .unwrap()
            .decrypt_padded_mut::<Pkcs7>(&mut buf)
            .map(|pt| pt.to_vec())?,
        n => return Err(anyhow::anyhow!("Unsupported CEK length: {n} bytes")),
    };

    Ok(plaintext)
}
