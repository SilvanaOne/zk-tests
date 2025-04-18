use mina_hasher::{Fp, Hashable, Hasher, ROInput, create_kimchi};
use std::time::Instant;
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

fn main() {
    println!("Calculating Poseidon hash...");
    let poseidon_input = PoseidonInput {
        a: Fp::from(1),
        b: Fp::from(2),
    };
    let mut hasher = create_kimchi::<PoseidonInput>(());
    let time_start = Instant::now();
    let mut i = 0;
    while i < 10000 {
        hasher.hash(&poseidon_input);
        i += 1;
    }
    let time_end = Instant::now();
    let duration = time_end.duration_since(time_start);
    let hash = hasher.hash(&poseidon_input);
    let hash_str = hash.to_string();
    println!("Time taken: {:?}", duration);
    println!("Hash: {:?}", hash_str);
}
