use mina_hasher::{Fp, Hashable, Hasher, ROInput, create_kimchi};

#[derive(Debug, Clone)]
pub struct PoseidonInput {
    pub data: [Fp; 3],
}

impl Hashable for PoseidonInput {
    type D = ();

    fn to_roinput(&self) -> ROInput {
        ROInput::new()
            .append_field(self.data[0])
            .append_field(self.data[1])
            .append_field(self.data[2])
    }

    fn domain_string(_: Self::D) -> Option<String> {
        //Some("CodaSignature".to_string().into())
        None
    }
}

#[allow(dead_code)]
pub fn poseidon_hash(data: &PoseidonInput, iterations: usize) -> String {
    let mut hasher = create_kimchi::<PoseidonInput>(());
    let mut i = 0;
    while i < iterations {
        hasher.hash(&data);
        i += 1;
    }
    let hash = hasher.hash(&data);
    hash.to_string()
}
