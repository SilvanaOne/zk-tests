use dotenvy::dotenv;
use mina_signer::{BaseField, Keypair, SecKey};

#[test]
fn sign_fields_1_2_3_and_print_base58() {
    let _ = dotenv();
    let sk_b58 =
        std::env::var("TEST_ACCOUNT_1_PRIVATE_KEY").expect("TEST_ACCOUNT_1_PRIVATE_KEY not set");
    let kp =
        Keypair::from_secret_key(SecKey::from_base58(&sk_b58).expect("invalid base58 secret key"))
            .expect("failed to build keypair");

    let fields = vec![
        BaseField::from(1u64),
        BaseField::from(2u64),
        BaseField::from(3u64),
    ];
    let sig = mina_keypair::signature::create_signature(&kp, &fields);
    let b58 = mina_keypair::signature::signature_to_base58(&sig);
    let strs = mina_keypair::signature::signature_to_strings(&sig);
    println!("signature_base58={}", b58);
    println!("r={} s={}", strs.r, strs.s);

    let addr =
        std::env::var("TEST_ACCOUNT_1_PUBLIC_KEY").expect("TEST_ACCOUNT_1_PUBLIC_KEY not set");
    let ok = mina_keypair::signature::verify_signature(&b58, &addr, &fields);
    println!("verified={}", ok);
    assert!(ok);
}
