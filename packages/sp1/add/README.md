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

## Requirements

- Rust with SP1 toolchain
- Foundry (for Ethereum)
- Sui CLI (for Sui)
- Anchor CLI (for Solana)
- Environment variables configured in `.env` file
