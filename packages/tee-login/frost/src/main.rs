mod ed25519;
mod secp256k1;

fn main() {
    println!("Starting Frost...");
    ed25519::frost_ed25519().unwrap();
    secp256k1::frost_secp256k1().unwrap();
}
