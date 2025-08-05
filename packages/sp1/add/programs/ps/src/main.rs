#![no_main]
sp1_zkvm::entrypoint!(main);
use mina_hasher::{create_kimchi, Fp, Hashable, Hasher, ROInput};

#[derive(Debug, Clone)]
struct PoseidonInput {
    a: Fp,
    b: Fp,
}

impl Hashable for PoseidonInput {
    type D = ();

    fn to_roinput(&self) -> ROInput {
        ROInput::new().append_field(self.a).append_field(self.b)
    }

    fn domain_string(_: Self::D) -> Option<String> {
        // format!("PoseidonInput").into()
        None
    }
}

pub fn main() {
    let iterations = sp1_zkvm::io::read::<u32>();
    let input = sp1_zkvm::io::read::<Vec<u32>>();
    assert!(iterations > 0, "Must have at least one iteration");

    let poseidon_input = PoseidonInput {
        a: Fp::from(input[0]),
        b: Fp::from(input[1]),
    };
    let mut hasher = create_kimchi::<PoseidonInput>(());

    // Perform hash calculations in the loop
    let mut digest = hasher.hash(&poseidon_input);
    for _ in 1..iterations {
        digest = hasher.hash(&poseidon_input);
    }

    sp1_zkvm::io::commit_slice(digest.to_string().as_bytes());
}
