use serde::Serialize;
mod keypair;
use crate::keypair::{bcs_serialize, generate_ed25519, sign_message, verify_with_address};

fn main() {
    let generated = generate_ed25519();
    println!("Sui Address: {}", generated.address);
    println!(
        "Secret Key (bech32 suiprivkey): {}",
        generated.suiprivkey_bech32
    );
    println!("Public Key (hex): {}", hex::encode(generated.public_key));

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
    let message = bcs_serialize(&payload).expect("bcs encode payload");

    // Sign using the generated private key
    let sui_sig = sign_message(&generated.secret_key, &message);

    let ok = verify_with_address(&generated.address, &message, &sui_sig);
    println!("Signature (hex): {}", hex::encode(&sui_sig));
    println!("Verified with address only: {}", ok);
}
