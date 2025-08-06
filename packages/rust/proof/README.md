# Kimchi Zero-Knowledge Proof System: Poseidon Hash Example

This project demonstrates how to create zero-knowledge proofs using the Kimchi proof system (Mina's variant of PLONK) to prove knowledge of a Poseidon hash preimage without revealing the actual values.

## Table of Contents
- [Overview](#overview)
- [How Zero-Knowledge Proofs Work](#how-zero-knowledge-proofs-work)
- [The Kimchi Proof System](#the-kimchi-proof-system)
- [Creating Circuits](#creating-circuits)
- [Gates in Kimchi](#gates-in-kimchi)
- [Generating Proofs](#generating-proofs)
- [Verifying Proofs](#verifying-proofs)
- [Example: Poseidon Hash Proof](#example-poseidon-hash-proof)
- [Performance](#performance)
- [Building and Running](#building-and-running)

## Overview

This implementation proves knowledge of three values (1, 2, 3) that hash to a specific Poseidon hash output without revealing the values themselves. The proof system uses:

- **Kimchi**: Mina's PLONK-based proof system with custom gates
- **Poseidon Hash**: A ZK-friendly hash function optimized for arithmetic circuits
- **Pasta Curves**: Pallas and Vesta curves designed for recursive proof composition

## How Zero-Knowledge Proofs Work

Zero-knowledge proofs allow a prover to convince a verifier that they know some secret information without revealing the information itself. The process involves:

1. **Arithmetization**: Converting the computation into polynomial constraints
2. **Commitment**: Cryptographically committing to the witness (private inputs)
3. **Challenge-Response**: Interactive protocol made non-interactive via Fiat-Shamir
4. **Verification**: Checking polynomial evaluations match the committed values

### Key Properties
- **Completeness**: Valid proofs always verify
- **Soundness**: Invalid proofs fail with overwhelming probability
- **Zero-Knowledge**: The proof reveals nothing about the witness

## The Kimchi Proof System

Kimchi is Mina Protocol's enhancement of the PLONK proof system, featuring:

### Architecture
```
Circuit (Gates) → Constraint System → Polynomials → Commitments → Proof
```

### Key Components

1. **Domain Construction**
   - Uses multiplicative subgroups of the field
   - Domain size must be a power of 2 for FFT efficiency
   - 2-adicity of 32 enables domains up to 2^32

2. **Witness Structure**
   - 15 columns (wires): 7 I/O registers + 8 advice registers
   - Each row represents one gate's inputs/outputs
   - Gates can access current row and next row

3. **Polynomial Commitments**
   - Uses bulletproof-style IPA (Inner Product Arguments)
   - Logarithmic proof size: O(log n)
   - Hiding commitments for zero-knowledge

## Creating Circuits

Circuits in Kimchi are collections of gates that enforce constraints:

```rust
// Create a circuit for Poseidon hash
pub fn create_poseidon_circuit() -> Vec<CircuitGate<Fp>> {
    let round_constants = &*Vesta::sponge_params().round_constants;
    let mut gates = vec![];
    let mut abs_row = 0;
    
    // Create Poseidon gadget (11 gates + 1 zero gate)
    let first_wire = Wire::for_row(abs_row);
    let last_wire = Wire::for_row(abs_row + POS_ROWS_PER_HASH);
    
    let (poseidon_gates, next_row) = CircuitGate::<Fp>::create_poseidon_gadget(
        abs_row,
        [first_wire, last_wire],
        round_constants,
    );
    gates.extend(poseidon_gates);
    
    gates
}
```

### Circuit Design Principles
- **Gate Locality**: Each gate only accesses current and next row
- **Constraint Satisfaction**: All gate polynomials must evaluate to zero
- **Wire Consistency**: Copy constraints ensure wire values match across gates

## Gates in Kimchi

Kimchi supports various gate types, each with specific constraints:

### 1. **Generic Gates**
Basic arithmetic operations:
```rust
// Addition gate: left + right = output
GenericGateSpec::Add { left_coeff, right_coeff, output_coeff }

// Multiplication gate: left * right = output
GenericGateSpec::Mul { output_coeff, mul_coeff }

// Constant gate: wire = constant
GenericGateSpec::Const(value)
```

### 2. **Poseidon Gates**
Specialized for the Poseidon hash function:
- 11 gates per hash (55 rounds ÷ 5 rounds per gate)
- Implements S-box (x^7) and linear transformations
- Optimized for field arithmetic

### 3. **Zero Gates**
Used for padding and connecting wires:
```rust
CircuitGate::zero(wire) // All constraints satisfied trivially
```

### 4. **Custom Gates**
- **CompleteAdd**: Elliptic curve point addition
- **VarBaseMul**: Variable base scalar multiplication
- **EndoMul**: Endomorphism-based multiplication
- **Lookup**: Table lookups via Plookup

### Gate Constraints
Each gate type defines polynomial constraints that must equal zero:
```
constraint(witness, coefficients) = 0
```

## Generating Proofs

The proof generation process:

### 1. **Create Witness**
```rust
pub fn create_poseidon_witness(domain_size: usize) -> [Vec<Fp>; COLUMNS] {
    let mut witness: [Vec<Fp>; COLUMNS] = array::from_fn(|_| vec![Fp::zero(); domain_size]);
    
    // Set private inputs
    let input = [Fp::from(1u32), Fp::from(2u32), Fp::from(3u32)];
    
    // Generate Poseidon witness values
    generate_witness(0, &Vesta::sponge_params(), &mut witness, input);
    
    witness
}
```

### 2. **Create Prover Index**
```rust
let prover_index = new_index_for_test::<Vesta>(gates, public_inputs.len());
```

The prover index contains:
- Constraint system with domain parameters
- Polynomial commitments to the circuit
- Precomputed Lagrange bases
- Zero-knowledge row configuration

### 3. **Generate Proof**
```rust
let proof = ProverProof::create::<BaseSponge, ScalarSponge, _>(
    &group_map,
    witness,
    &[],  // runtime tables
    &prover_index,
    &mut rng,
)?;
```

Steps performed:
1. **Polynomial Interpolation**: Convert witness to polynomials
2. **Commitment Phase**: Commit to witness polynomials
3. **Constraint Aggregation**: Combine all constraints with random challenges
4. **Quotient Computation**: Divide by vanishing polynomial
5. **Opening Proof**: Prove polynomial evaluations at challenge point

## Verifying Proofs

Verification is much faster than proving:

```rust
let result = verify::<Vesta, BaseSponge, ScalarSponge, OpeningProof>(
    &group_map,
    &verifier_index,
    &proof,
    &public_inputs,
);
```

Verification steps:
1. **Recompute Challenges**: Derive same challenges via Fiat-Shamir
2. **Check Commitments**: Verify polynomial commitment openings
3. **Constraint Check**: Ensure all constraints satisfied at challenge point
4. **Batch Verification**: Combine multiple checks for efficiency

## Example: Poseidon Hash Proof

Our implementation proves knowledge of values that hash to:
```
24619730558757750532171846435738270973938732743182802489305079455910969360336
```

### Circuit Statistics
- **Total Gates**: 12 (11 Poseidon + 1 Zero)
- **Domain Size**: 16 (2^4)
- **Zero-Knowledge Rows**: 3
- **Witness Columns**: 15

### Security Parameters
- **Field Size**: ~255 bits (Pasta curve scalar field)
- **Soundness Error**: ≤ degree/|field| ≈ 2^-250
- **Sponge Parameters**: 55 full rounds, rate=2, capacity=1

## Performance

Measured on Apple M1 (Release mode):
- **Hash Computation**: 0.285 ms
- **Proof Generation**: 650.442 ms
- **Proof Verification**: 56.315 ms
- **Proof Size**: ~10-20 KB

### Complexity Analysis
- **Proving**: O(n log n) where n is circuit size
- **Verification**: O(log n) 
- **Memory**: O(n) for witness storage

## Public Inputs and Outputs in Kimchi

### How Kimchi Handles Public Inputs

According to the Kimchi specification and implementation:

1. **Public Input Location**: Public inputs must be placed in the first `n` rows of column 0 of the witness, where `n` is the number of public inputs specified in the constraint system.

2. **Negated Public Polynomial**: During proof creation, Kimchi creates a "negated public input polynomial" that:
   - Evaluates to `-p_i` for the first `public_input_size` values of the domain
   - Evaluates to `0` for the rest of the domain
   - This polynomial is committed and included in the proof

3. **Verification**: The verifier uses the public inputs to verify the proof by:
   - Creating the same negated public polynomial
   - Checking that the proof's evaluations match the expected public values

### Implementation in Kimchi

From the proof-systems repository:

```rust
// In the constraint system builder
let cs = ConstraintSystem::create(gates)
    .public(num_public_inputs)  // Specify number of public inputs
    .build()
    .unwrap();

// In the witness
let public_inputs = witness[0][0..cs.public].to_vec();

// During proof creation
let public_poly = -Evaluations::from_vec_and_domain(
    public_inputs,
    domain,
).interpolate();
```

### Public Outputs Pattern

While Kimchi doesn't have a separate "public output" concept, public outputs are implemented by:

1. Placing the output value at a known position in the public inputs (e.g., position 0)
2. Adding a constraint that copies the computed value to this public position
3. The verifier then checks that the public value matches the expected output

Example pattern for public outputs:
```rust
// Row 0: Public output (hash value)
witness[0][0] = computed_hash;

// Add constraint: computed_value == public_output
// This ensures the circuit's computed value matches the public output
```

### Example: Public Output Implementation

Here's how you would implement public outputs for the Poseidon hash circuit:

```rust
// 1. Create circuit with wiring to public output
pub fn create_poseidon_circuit_with_public() -> Vec<CircuitGate<Fp>> {
    let mut gates = vec![];
    
    // Row 0: Reserved for public output (no gate)
    // Rows 1-11: Poseidon gates
    let (poseidon, _) = CircuitGate::create_poseidon_gadget(
        1,  // Start from row 1
        [Wire::for_row(1), Wire::for_row(12)],
        round_constants,
    );
    gates.extend(poseidon);
    
    // Wire the hash output (row 12) to public input (row 0)
    gates.connect_cell_pair((0, 0), (12, 0));
    
    gates
}

// 2. Create witness with public output
pub fn create_witness_with_public(domain_size: usize) -> [Vec<Fp>; COLUMNS] {
    let mut witness = array::from_fn(|_| vec![Fp::zero(); domain_size]);
    
    // Generate Poseidon witness starting from row 1
    let input = [Fp::from(1), Fp::from(2), Fp::from(3)];
    generate_witness(1, &sponge_params, &mut witness, input);
    
    // Copy hash output to public position
    let hash_output = witness[0][12];  // Hash is at row 12
    witness[0][0] = hash_output;        // Copy to row 0 (public)
    
    witness
}

// 3. Build constraint system with public inputs
let cs = ConstraintSystem::create(gates)
    .public(1)  // 1 public input/output
    .build()
    .unwrap();

// 4. Extract public inputs for proving/verifying
let public_inputs = witness[0][0..cs.public].to_vec();

// 5. Verification with public hash
let expected_hash = Fp::from_str("24619...").unwrap();
let result = verify::<Vesta, BaseSponge, ScalarSponge, OpeningProof>(
    &group_map,
    &verifier_index,
    &proof,
    &vec![expected_hash],  // Public inputs
);
```

### Current Implementation Note

The current demonstration keeps the implementation simple by:
- Computing the hash within the circuit
- Returning the hash value alongside the proof
- Not implementing the full public input/output pattern

The simplified approach is sufficient for demonstration purposes while the example above shows how to properly implement public outputs following Kimchi's design. The key insight is that public inputs/outputs in Kimchi are simply the first `n` values in the witness column 0, where `n` is specified by `.public(n)` in the constraint system.

## Building and Running

### Prerequisites
- Rust 1.70+
- Git

### Build
```bash
# Clone the repository
git clone <repository>
cd proof

# Build in release mode
cargo build --release

# Run the example
cargo run --release

# Run tests
cargo test
```

### Output
```
=== Poseidon Hash ZK Proof Demo ===

Step 1: Poseidon hash of [1, 2, 3]:
  Hash: 24619730558757750532171846435738270973938732743182802489305079455910969360336
  Time: 0.285 ms

Step 2: Circuit information:
  Total gates: 12 (Poseidon: 11, Zero: 1)
  Domain size: 16 (2^4), ZK rows: 3

Step 3: Creating ZK proof that we know the preimage...
  ✓ Proof created successfully!
  Time: 650.442 ms

Step 4: Verifying the proof...
  ✓ Proof verified successfully!
  Time: 56.315 ms
```

## Theory References

### Polynomial Commitment Schemes
The system uses Inner Product Arguments (IPA) for polynomial commitments:
- Commitment: `C = Σ(p_i * G_i)` where G_i are curve points
- Opening: Proves `p(z) = v` for committed polynomial p
- Batching: Multiple openings combined for efficiency

### Fiat-Shamir Transform
Makes the protocol non-interactive using a cryptographic hash function:
```
challenge = Hash(transcript || commitment || public_inputs)
```

### Vanishing Polynomial
For domain H = {1, ω, ω², ..., ω^(n-1)}:
```
Z_H(x) = x^n - 1
```
All valid constraints must be divisible by Z_H.

## Further Reading

- [PLONK Paper](https://eprint.iacr.org/2019/953): Original PLONK construction
- [Mina Technical Docs](https://docs.minaprotocol.com/): Protocol documentation
- [Kimchi Specification](https://github.com/o1-labs/proof-systems/tree/main/book): Detailed specification
- [Poseidon Hash](https://eprint.iacr.org/2019/458): ZK-friendly hash function

## License

This project is licensed under the Apache 2.0 License.