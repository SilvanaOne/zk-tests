use dotenvy::dotenv;
use mina_signer::{Keypair, SecKey};
use std::env;

#[test]
fn keypair_from_env_matches_public_address() {
    let _ = dotenv();

    let private_b58 =
        env::var("TEST_ACCOUNT_1_PRIVATE_KEY").expect("TEST_ACCOUNT_1_PRIVATE_KEY not set");
    let expected_address =
        env::var("TEST_ACCOUNT_1_PUBLIC_KEY").expect("TEST_ACCOUNT_1_PUBLIC_KEY not set");

    let sec_key = SecKey::from_base58(&private_b58).expect("invalid base58 secret key");
    let keypair = Keypair::from_secret_key(sec_key).expect("failed to build keypair");

    let address = keypair.public.into_address();
    println!("address         : {}", address);
    println!("expected address: {}", expected_address);
    assert_eq!(address, expected_address);
}
