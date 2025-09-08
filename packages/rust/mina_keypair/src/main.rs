use mina_signer::Keypair;
use rand::rngs::OsRng;

mod signature;

fn main() {
    let kp = Keypair::rand(&mut OsRng).expect("failed to generate keypair");
    let private_b58 = kp.secret.to_base58();
    let public_addr = kp.public.into_address();

    println!("TEST_ACCOUNT_1_PRIVATE_KEY={}", private_b58);
    println!("TEST_ACCOUNT_1_PUBLIC_KEY={}", public_addr);
}
