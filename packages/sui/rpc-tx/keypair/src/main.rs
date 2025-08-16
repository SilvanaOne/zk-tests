use blake2::{Blake2b512, Digest};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::Serialize;
use sui_crypto::ed25519::Ed25519PrivateKey;
use sui_sdk_types as sui;

fn main() {
    let mut csprng = OsRng;
    let mut sk_bytes = [0u8; 32];
    csprng.fill_bytes(&mut sk_bytes);
    let sk = Ed25519PrivateKey::new(sk_bytes);

    // Derive public key from private key
    let pk = sk.public_key();

    // Compute Sui address: blake2b256( flag || pubkey )[0..32]
    let mut hasher = Blake2b512::new();
    hasher.update([0x00u8]); // ed25519 scheme flag
    hasher.update(pk.as_bytes());
    let full = hasher.finalize();
    let mut addr_bytes = [0u8; 32];
    addr_bytes.copy_from_slice(&full[..32]);
    let address = sui::Address::from_bytes(&addr_bytes).expect("address length");

    // Export the secret key as bech32 suiprivkey (flag 0x00 || 32-byte secret)
    let secret_bytes = &sk_bytes;
    let mut payload = Vec::with_capacity(33);
    payload.push(0x00);
    payload.extend_from_slice(secret_bytes);
    let bech = bech32::encode(
        "suiprivkey",
        bech32::ToBase32::to_base32(&payload),
        bech32::Variant::Bech32,
    )
    .expect("bech32 encode");

    println!("Sui Address: {}", address);
    println!("Secret Key (bech32 suiprivkey): {}", bech);

    // Define a struct: two strings and two u64 numbers
    #[derive(Serialize)]
    struct MyPayload {
        first: String,
        second: String,
        number_one: u64,
        number_two: u64,
    }

    let payload = MyPayload {
        first: "hello".to_string(),
        second: "sui".to_string(),
        number_one: 42,
        number_two: 7,
    };

    // BCS-encode the payload
    let message = bcs::to_bytes(&payload).expect("bcs encode payload");

    // Sign using the generated private key
    let signing_key = SigningKey::from_bytes(&sk_bytes);
    let signature: Signature = signing_key.sign(&message);

    // Construct Sui-style user signature bytes: [flag || signature || public_key]
    let verifying_key = signing_key.verifying_key();
    let mut sui_sig = Vec::with_capacity(1 + 64 + 32);
    sui_sig.push(0x00); // ed25519 flag
    sui_sig.extend_from_slice(&signature.to_bytes());
    sui_sig.extend_from_slice(verifying_key.as_bytes());

    // Verify using only address + message + sui_sig (no secret key)
    fn verify_with_address(addr: &sui::Address, msg: &[u8], sui_sig: &[u8]) -> bool {
        if sui_sig.len() != 97 {
            return false;
        }
        if sui_sig[0] != 0x00 {
            return false;
        }
        let sig_bytes: [u8; 64] = match sui_sig[1..65].try_into() {
            Ok(b) => b,
            Err(_) => return false,
        };
        let pk_bytes: [u8; 32] = match sui_sig[65..97].try_into() {
            Ok(b) => b,
            Err(_) => return false,
        };

        // Recompute address from [flag || pubkey]
        let mut hasher = Blake2b512::new();
        hasher.update([0x00u8]);
        hasher.update(&pk_bytes);
        let full = hasher.finalize();
        let mut addr_bytes = [0u8; 32];
        addr_bytes.copy_from_slice(&full[..32]);
        let derived = sui::Address::from_bytes(&addr_bytes).ok();
        if derived.as_ref() != Some(addr) {
            return false;
        }

        // Verify ed25519 signature
        let vk = VerifyingKey::from_bytes(&pk_bytes).ok();
        let sig = Signature::from_bytes(&sig_bytes);
        match vk {
            Some(vk) => vk.verify(msg, &sig).is_ok(),
            None => false,
        }
    }

    let ok = verify_with_address(&address, &message, &sui_sig);
    println!("Signature (hex): {}", hex::encode(&sui_sig));
    println!("Verified with address only: {}", ok);
}
