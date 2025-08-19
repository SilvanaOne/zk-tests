use serde::Serialize;
mod keypair;
use crate::keypair::{
    bcs_serialize, from_sui_private_key, generate_ed25519, sign_message, verify_with_address,
};
use std::env;

fn test_env_keypair() {
    dotenv::dotenv().ok();

    let sui_secret_key = env::var("SUI_SECRET_KEY").expect("SUI_SECRET_KEY not found in .env");
    let expected_address = env::var("SUI_ADDRESS").expect("SUI_ADDRESS not found in .env");

    let keypair = from_sui_private_key(&sui_secret_key)
        .expect("Failed to create keypair from SUI_SECRET_KEY");

    println!("\n--- Environment Test ---");
    println!("Expected Address: {}", expected_address);
    println!("Derived Address:  {}", keypair.address);
    println!(
        "Addresses match: {}",
        keypair.address.to_string() == expected_address
    );

    assert_eq!(
        keypair.address.to_string(),
        expected_address,
        "Address mismatch!"
    );
    println!("✓ Environment test passed!");
}

fn main() {
    let generated = generate_ed25519();
    println!("Generated Sui Address: {}", generated.address);
    println!(
        "Generated Secret Key (bech32 suiprivkey): {}",
        generated.sui_private_key
    );
    println!(
        "Generated Public Key (hex): {}",
        hex::encode(generated.public_key)
    );

    // Test using the generated sui_private_key to recreate the keypair
    let recreated = from_sui_private_key(&generated.sui_private_key)
        .expect("Failed to recreate keypair from sui_private_key");

    println!("\n--- Verification ---");
    println!("Recreated Sui Address: {}", recreated.address);
    println!(
        "Addresses match: {}",
        generated.address == recreated.address
    );

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

    // Sign using the recreated private key to verify it works
    let sui_sig = sign_message(&recreated.secret_key, &message);

    let ok = verify_with_address(&recreated.address, &message, &sui_sig);
    println!("Signature (hex): {}", hex::encode(&sui_sig));
    println!("Verified with recreated keypair: {}", ok);

    // Test environment keypair
    test_env_keypair();
}
