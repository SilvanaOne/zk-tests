mod hash;
mod keypair;
mod secrets;
mod signer;

use crate::hash::{PoseidonInput, poseidon_hash};
use crate::keypair::generate_keypair;
use crate::signer::{sign_fields, verify_fields};
use mina_hasher::Fp;
use mina_signer::{Keypair, SecKey};
use secrets::{add, remove, with_secret};
use std::str::FromStr;
use std::time::Instant;

fn main() {
    let _ = generate_keypair();

    let secret_key =
        SecKey::from_base58("EKEtdWo2dFqNx6qEhbeVaHkYbcnLhz3pXkS4mjQyfJdPFZB3onG5").unwrap();
    let keypair = Keypair::from_secret_key(secret_key).unwrap();
    let address = &keypair.clone().get_address();
    println!("Address: {:?}", address);
    let private_key = &keypair.clone().secret;
    let private_key_str = private_key.to_base58();
    println!("Private key: {:?}", private_key_str);
    let public_key = &keypair.clone().public;
    let public_key_str = public_key.to_string();
    println!("Public key: {:?}", public_key_str);
    let msg = PoseidonInput {
        data: [
            Fp::from_str("240717916736854602989207148466022993262069182275").unwrap(),
            Fp::from(1),
            Fp::from(2), //Fp::from(3),
        ],
    };
    let signature = sign_fields(&msg, &keypair);
    println!("Signature: {:?}", signature);
    let signature_str = signature.to_string();
    println!("Signature string: {:?}", signature_str);
    let ok = verify_fields(&msg, &keypair.public, &signature);
    println!("Signature verified: {:?}", ok);
    println!("Calculating Poseidon hash...");
    let time_start = Instant::now();
    let hash = poseidon_hash(&msg, 100);
    let time_end = Instant::now();
    let duration = time_end.duration_since(time_start);
    println!("Time taken: {:?}", duration);
    println!("Hash: {:?}", hash);
    add("alice", b"top-secret-bytes".to_vec());

    // Use
    with_secret("alice", |bytes| {
        println!("length = {}", bytes.len());
        // sign_message(bytes);
    });

    // Rotate / delete
    remove("alice");
}
