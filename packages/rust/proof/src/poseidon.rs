use mina_poseidon::{
    constants::PlonkSpongeConstantsKimchi,
    pasta::fp_kimchi,
    poseidon::{ArithmeticSponge as Poseidon, Sponge as _},
};
use mina_hasher::Fp;

pub fn poseidon_hash(values: &[Fp]) -> Fp {
    let mut sponge = Poseidon::<Fp, PlonkSpongeConstantsKimchi>::new(
        fp_kimchi::static_params()
    );
    sponge.absorb(values);
    sponge.squeeze()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    
    #[test]
    fn test_poseidon_hash_123() {
        let values = vec![
            Fp::from(1u64),
            Fp::from(2u64),
            Fp::from(3u64),
        ];
        
        let hash = poseidon_hash(&values);
        let expected = Fp::from_str("24619730558757750532171846435738270973938732743182802489305079455910969360336")
            .expect("Failed to parse expected hash");
        
        assert_eq!(hash, expected, "Hash of [1, 2, 3] should match expected value");
    }
}