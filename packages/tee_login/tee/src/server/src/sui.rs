use base64::{Engine, engine::general_purpose::STANDARD as B64};
use fastcrypto::ed25519::{Ed25519PublicKey, Ed25519Signature};
use fastcrypto::hash::{Blake2b256, HashFunction};
use fastcrypto::traits::ToFromBytes;
use fastcrypto::traits::VerifyingKey;
use shared_crypto::intent::{Intent, IntentMessage, PersonalMessage};

// pub struct Ed25519SuiSignature(
//     #[schemars(with = "Base64")]
//     #[serde_as(as = "Readable<Base64, Bytes>")]
//     [u8; Ed25519PublicKey::LENGTH + Ed25519Signature::LENGTH + 1],
// );
// first byte - 0 for ed25519, 1 for secp256k1
// next 64 bytes - signature
// next 32 bytes - public key

pub async fn verify_signature(
    address: &str,
    signature_base64: &str,
    message: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    println!(
        "Sui verify signature: {:?}",
        (address, signature_base64, message)
    );
    let bytes = message.as_bytes();
    if !address.starts_with("0x") || address.len() != 66 {
        return Err("Invalid address format".into());
    }

    let ok = verify_personal_message_signature(signature_base64, bytes, address)
        .await
        .is_ok();
    println!("Sui verify signature result: {:?}", ok);
    Ok(ok)
}

pub async fn verify_personal_message_signature(
    signature_base64: &str,
    message: &[u8],
    address: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let intent_msg = IntentMessage::new(
        Intent::personal_message(),
        PersonalMessage {
            message: message.to_vec(),
        },
    );
    let ok = verify_secure(signature_base64, &intent_msg, address).is_ok();
    Ok(ok)
}

fn verify_secure(
    signature_base64: &str,
    value: &IntentMessage<PersonalMessage>,
    address: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let mut hasher = Blake2b256::default();
    hasher.update(bcs::to_bytes(&value).expect("Message serialization should not fail"));
    let digest = hasher.finalize().digest;

    let decoded = B64.decode(signature_base64).unwrap();

    // Sui signature format: [scheme_byte][64_bytes_signature][32_bytes_pubkey]
    if decoded.len() != 97 {
        return Err("Invalid signature length".into());
    }

    let scheme_byte = decoded[0];

    if scheme_byte != 0 {
        println!("scheme_byte: {:?}", scheme_byte);
        return Err("Only Ed25519 signatures supported".into());
    }

    let sig_bytes = &decoded[1..65]; // 64 bytes signature
    let pk_bytes = &decoded[65..97]; // 32 bytes public key

    let sig = Ed25519Signature::from_bytes(sig_bytes).unwrap();
    let pk = Ed25519PublicKey::from_bytes(pk_bytes).unwrap();

    let recovered_address = get_address(&pk);

    if recovered_address != address {
        println!("recovered_address: {:?}", recovered_address);
        println!("address: {:?}", address);
        return Ok(false);
    }

    Ok(pk.verify(&digest, &sig).is_ok())
}

fn get_address(pk: &Ed25519PublicKey) -> String {
    let mut hasher = Blake2b256::default();

    hasher.update([0]); // ed25519 flag
    hasher.update(pk);
    let address = hasher.finalize().digest;
    format!("0x{}", hex::encode(address))
}
