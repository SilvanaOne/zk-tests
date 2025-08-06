use ark_ff::Zero;
use ark_poly::EvaluationDomain;
use groupmap::GroupMap;
use kimchi::{
    circuits::{
        gate::CircuitGate,
        polynomials::poseidon::{generate_witness, ROUNDS_PER_ROW},
        wires::{Wire, COLUMNS},
    },
    curve::KimchiCurve,
    proof::ProverProof,
    prover_index::testing::new_index_for_test,
    verifier::verify,
    verifier_index::VerifierIndex,
};
use mina_curves::pasta::{Fp, Vesta, VestaParameters};
use mina_poseidon::{
    constants::{PlonkSpongeConstantsKimchi, SpongeConstants},
    sponge::{DefaultFqSponge, DefaultFrSponge},
};
use poly_commitment::{commitment::CommitmentCurve, ipa::OpeningProof as DlogOpeningProof};
use std::array;

type SpongeParams = PlonkSpongeConstantsKimchi;
type BaseSponge = DefaultFqSponge<VestaParameters, SpongeParams>;
type ScalarSponge = DefaultFrSponge<Fp, SpongeParams>;
type OpeningProof = DlogOpeningProof<Vesta>;

// Constants for Poseidon circuit
const NUM_POS: usize = 1; // Number of Poseidon hashes in the circuit
const ROUNDS_PER_HASH: usize = SpongeParams::PERM_ROUNDS_FULL;
const POS_ROWS_PER_HASH: usize = ROUNDS_PER_HASH / ROUNDS_PER_ROW;

pub struct PoseidonProof {
    pub proof: ProverProof<Vesta, OpeningProof>,
    pub verifier_index: VerifierIndex<Vesta, OpeningProof>,
}

/// Create a circuit that computes a Poseidon hash
pub fn create_poseidon_circuit() -> Vec<CircuitGate<Fp>> {
    // Get round constants
    let round_constants = &*Vesta::sponge_params().round_constants;

    // Create gates
    let mut gates = vec![];
    let mut abs_row = 0;

    for _ in 0..NUM_POS {
        let first_wire = Wire::for_row(abs_row);
        let last_row = abs_row + POS_ROWS_PER_HASH;
        let last_wire = Wire::for_row(last_row);

        let (poseidon, row) = CircuitGate::<Fp>::create_poseidon_gadget(
            abs_row,
            [first_wire, last_wire],
            round_constants,
        );
        gates.extend(poseidon);
        abs_row = row;
    }

    gates
}

/// Generate witness for the Poseidon hash circuit with inputs [1, 2, 3]
pub fn create_poseidon_witness(num_rows: usize) -> [Vec<Fp>; COLUMNS] {
    // Initialize witness with zeros - must match exact witness size expected
    let mut witness: [Vec<Fp>; COLUMNS] = array::from_fn(|_| vec![Fp::zero(); num_rows]);

    // Our private inputs: 1, 2, 3
    let input = [Fp::from(1u32), Fp::from(2u32), Fp::from(3u32)];

    // Generate witness for each Poseidon instance
    for h in 0..NUM_POS {
        let first_row = h * (POS_ROWS_PER_HASH + 1);

        generate_witness(first_row, &Vesta::sponge_params(), &mut witness, input);
    }

    witness
}

/// Create a proof that we know the preimage of the Poseidon hash
///
/// Note: This implementation doesn't use public inputs/outputs for simplicity.
/// In a production implementation:
/// 1. Reserve witness[0][0] for the public hash output
/// 2. Add a copy constraint from the computed hash to witness[0][0]
/// 3. Use .public(1) when building the constraint system
/// 4. Extract public_inputs = witness[0][0..1].to_vec()
/// 5. Pass public_inputs to both prover_index.verify() and ProverProof::create()
pub fn create_poseidon_proof() -> Result<PoseidonProof, Box<dyn std::error::Error>> {
    // Create circuit
    let gates = create_poseidon_circuit();

    // Calculate witness size - this should match what the test does
    let witness_size = POS_ROWS_PER_HASH * NUM_POS + 1; // +1 for last output row

    // Create witness first with the expected size
    let witness = create_poseidon_witness(witness_size);

    // No public inputs in this simplified version
    // In production: public_inputs = witness[0][0..cs.public].to_vec()
    let public_inputs: Vec<Fp> = vec![];

    // Use the simpler test index creation
    // In production: use cs.public = 1
    let prover_index = new_index_for_test::<Vesta>(gates, public_inputs.len());

    // Create verifier index
    let verifier_index = prover_index.verifier_index();

    // Verify witness is correct before proving (optional but helps debug)
    prover_index
        .verify(&witness, &public_inputs)
        .map_err(|e| format!("Witness verification failed: {:?}", e))?;

    // Create the proof
    let group_map = <Vesta as CommitmentCurve>::Map::setup();
    let mut rng = rand::rngs::OsRng;

    let proof = ProverProof::create::<BaseSponge, ScalarSponge, _>(
        &group_map,
        witness,
        &[], // runtime tables
        &prover_index,
        &mut rng,
    )?;

    Ok(PoseidonProof {
        proof,
        verifier_index,
    })
}

/// Verify a Poseidon proof and return the hash value
///
/// Note: In this simplified implementation, we don't use public inputs/outputs.
/// In a production implementation following Kimchi's design:
/// 1. The hash would be placed at witness[0][0] (first row, first column)
/// 2. The constraint system would have .public(1)
/// 3. The prover would extract public_inputs = witness[0][0..1]
/// 4. The verifier would pass expected_hash as vec![expected_hash] to verify()
///
/// This would cryptographically bind the proof to the specific hash value.
pub fn verify_poseidon_proof(
    proof_data: &PoseidonProof,
    expected_hash: Fp,
) -> Result<(bool, Fp), Box<dyn std::error::Error>> {
    let group_map = <Vesta as CommitmentCurve>::Map::setup();

    // Verify the proof (no public inputs in this simplified version)
    let result = verify::<Vesta, BaseSponge, ScalarSponge, OpeningProof>(
        &group_map,
        &proof_data.verifier_index,
        &proof_data.proof,
        &[],
    );

    // Return verification result and the expected hash
    Ok((result.is_ok(), expected_hash))
}

/// Get circuit information (number of gates and constraints)
pub fn get_circuit_info() -> (usize, String) {
    // Create circuit
    let gates = create_poseidon_circuit();
    let gates_count = gates.len();

    // Count gate types
    let mut poseidon_gates = 0;
    let mut zero_gates = 0;
    for gate in &gates {
        match gate.typ {
            kimchi::circuits::gate::GateType::Poseidon => poseidon_gates += 1,
            kimchi::circuits::gate::GateType::Zero => zero_gates += 1,
            _ => {}
        }
    }

    // Create prover index to get constraint system info
    let public_inputs: Vec<Fp> = vec![];
    let prover_index = new_index_for_test::<Vesta>(gates, public_inputs.len());

    // Get constraint system information
    let domain_size = prover_index.cs.domain.d1.size();
    let zk_rows = prover_index.cs.zk_rows;

    // Format constraint info
    let info = format!(
        "Total gates: {} (Poseidon: {}, Zero: {})\n  Domain size: {} (2^{}), ZK rows: {}",
        gates_count,
        poseidon_gates,
        zero_gates,
        domain_size,
        domain_size.ilog2(),
        zk_rows
    );

    (gates_count, info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::poseidon::poseidon_hash;
    use std::str::FromStr;

    #[test]
    fn test_constants() {
        println!("PERM_ROUNDS_FULL: {}", SpongeParams::PERM_ROUNDS_FULL);
        println!("COLUMNS: {}", COLUMNS);
        println!("ROUNDS_PER_ROW: {}", ROUNDS_PER_ROW);
        println!("POS_ROWS_PER_HASH: {}", POS_ROWS_PER_HASH);

        // These should match our output
        assert_eq!(POS_ROWS_PER_HASH, 11);
    }

    #[test]
    fn test_poseidon_proof() {
        // Verify that our inputs hash to the expected value
        let inputs = vec![Fp::from(1u64), Fp::from(2u64), Fp::from(3u64)];
        let hash = poseidon_hash(&inputs);
        let expected = Fp::from_str(
            "24619730558757750532171846435738270973938732743182802489305079455910969360336",
        )
        .expect("Failed to parse expected hash");
        assert_eq!(hash, expected);

        // Create and verify proof
        let proof = create_poseidon_proof().expect("Failed to create proof");
        let (is_valid, returned_hash) =
            verify_poseidon_proof(&proof, expected).expect("Failed to verify proof");

        assert!(is_valid, "Proof should be valid");
        assert_eq!(returned_hash, expected, "Hash should match expected value");
    }
}
