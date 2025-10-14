# SP1 Recursive Proof Aggregation Example

This repository demonstrates how to calculate recursive SP1 proofs, aggregate them, and verify them on multiple blockchains including Ethereum, Sui, and Solana. The example includes comprehensive utilities in a Makefile to handle the entire flow from proof generation to on-chain verification.

## Overview

The project showcases a simple addition operation where multiple values are added together, but the key feature is the recursive proof aggregation system that allows:

- Generating multiple SP1 proofs that form a chain (where each proof's output becomes the next proof's input)
- Aggregating these proofs into a single proof
- Converting the aggregated proof to Groth16 format for efficient on-chain verification
- Verifying the final proof on Ethereum, Sui, and Solana blockchains

## SP1 Proof Aggregation Process

The SP1 proof aggregation follows a multi-level approach:

### 1. First Level - Core Proof Generation

Multiple values are calculated in the core proof program (`programs/add`), creating the first level of recursion. Each proof:

- Takes an `old_sum` and an array of values as input
- Computes the sum of all values plus the old sum
- Outputs a `new_sum` that becomes the `old_sum` for the next proof
- This creates a chain of proofs where each proof depends on the previous one

### 2. Proof Compression

Once generated, the core proofs are compressed using SP1's compression algorithm. This reduces the proof size while maintaining its cryptographic validity, making it more efficient for aggregation.

### 3. Aggregation and Groth16 Conversion

The compressed proofs are then aggregated using the aggregate program (`programs/aggregate`):

- The aggregate program verifies all individual proofs using SP1's recursive verification
- It ensures the proofs form a valid chain (each proof's old_sum matches the previous proof's new_sum)
- The aggregated proof is then converted to a Groth16 proof, which is highly efficient for on-chain verification
- The final Groth16 proof contains the aggregated values (first old_sum and last new_sum)

### 4. On-Chain Settlement

The Groth16 proof, containing many aggregated values, is settled to various blockchains:

- **Ethereum**: Using a Solidity smart contract with SP1's Groth16 verifier
- **Sui**: Using Move smart contracts with native SP1 verification support
- **Solana**: Using Anchor framework with SP1 verification integration

## Project Structure

```
.
├── programs/             # SP1 programs
│   ├── add/              # Core addition program
│   └── aggregate/        # Proof aggregation program
├── script/               # Rust scripts for proof generation and verification
├── ethereum/             # Ethereum smart contracts
├── sui/                  # Sui Move contracts and client
├── solana/               # Solana Anchor program
├── lib/                  # Shared library code
└── Makefile              # Build and deployment utilities
```

## Usage

The Makefile provides comprehensive commands for the entire workflow:

### Building

```bash
make build              # Build the SP1 programs
```

### Proof Generation

```bash
make prove-core LENGTH=10 PROOFS=2      # Generate core SP1 proofs
make prove-groth16 LENGTH=10 PROOFS=2   # Generate and aggregate to Groth16
make prove-plonk LENGTH=10 PROOFS=2     # Generate and aggregate to PLONK
```

### Deployment

```bash
make deploy-groth16     # Deploy Ethereum contract with Groth16 verifier
make deploy-sui         # Deploy Sui Move contract
make deploy-solana      # Deploy Solana Anchor program
```

### Verification

```bash
make verify-groth16     # Verify Groth16 proof on Ethereum
make sui                # Run Sui client for verification
make solana             # Run Solana verification test
```

## Technical Details

### Core Components

1. **Add Program**: Performs the basic addition operation and generates proofs
2. **Aggregate Program**: Verifies multiple proofs recursively and aggregates them
3. **Smart Contracts**: On-chain verifiers for each blockchain platform
4. **Scripts**: Rust utilities for proof generation, aggregation, and verification

### Proof Flow

1. Generate multiple proofs with chained inputs/outputs
2. Compress proofs for efficient aggregation
3. Aggregate proofs recursively in SP1
4. Convert aggregated proof to Groth16
5. Submit Groth16 proof to blockchain for verification

## SP1 Recursive Proofs - Silvana Integration

### Ready Code for SP1 Integration in Silvana

- Recursive aggregated proofs - Groth16, PLONK
- Settlement on Ethereum, Sui, Solana
- Integration with SP1 proving service

Based on SP1-solana and soundness sp1-sui implementations (their code is practically identical).

### SP1 Features

SP1 has unique characteristics: recursive proofs can be computed directly inside the core proof using loops with variable iteration counts (impossible in Mina, where loops must have a fixed iteration count). The core proof performs folding automatically when operation limits are exceeded, but the limit is quite high, so folding wasn't reached in our tests. Computation time is 5-6 seconds, with minimal dependency on the number of user actions inside.

### Performance Stages

1. **Core Proof Generation**: 5-6 seconds
2. **Compression**: ~5 seconds (required as only compressed proofs can be aggregated)
3. **Aggregation**: 30-200 seconds depending on the number of proofs
4. **STARK to Groth16 Conversion**: 3-4 minutes (only needed for settlement on Sui/Solana/Ethereum)

When using SP1 Prover Service with GPU, Groth16 conversion takes only 20-30 seconds and costs $0.05-$0.20, depending on the number of aggregated proofs.

### Hardware Requirements

- Core proofs: ~10 GB RAM
- Groth16 proofs: ~32 GB RAM

### Optimal Recursion Strategy

The optimal recursion strategy differs from Mina and consists of:

1. Maximizing user actions within core proof recursion (fast and cheap)
2. Using direct proof aggregation only when necessary
3. Adding Groth16 conversion only for settlement (expensive and slow)

## Requirements

- Rust with SP1 toolchain
- Foundry (for Ethereum)
- Sui CLI (for Sui)
- Anchor CLI (for Solana)
- Environment variables configured in `.env` file

## Benchmarking on M2

2 proofs, 10 operations each

### Core proof - 344 seconds

=== Performance Summary ===
Setup time: 7.31s
Individual proofs: 252.01s
Aggregation: 85.07s
Verification: 0.08s
Total time: 344.47s

### Groth16 proof - 326 seconds

=== Performance Summary - groth16 generated in sp1 cloud for 0.05 USD ===
Setup time: 7.22s
Individual proofs: 237.44s
Aggregation: 26.24s (cloud)
Verification: 0.34s
Total time: 271.24s

=== Performance Summary - cpu ===
Setup time: 7.28s
Individual proofs: 262.24s
Aggregation: 435.95s
Verification: 0.40s
Total time: 705.88s

### Plonk proof - 472 seconds

=== Performance Summary - plonk generated in sp1 cloud for 0.05 USD ===
Setup time: 7.53s
Individual proofs: 348.01s
Aggregation: 116.53s
Verification: 0.50s
Total time: 472.56s

### Shrink proof - 351 seconds

=== Performance Summary ===
Setup time: 7.42s
Individual proofs: 255.73s
Aggregation: 81.69s
Shrink: 6.65s
Verification: 0.00s
Total time: 351.48s
