# SP1 hash benchmark

Measures SHA-256, Keccak-256 and Poseidon2 for hashing inside the [SP1](https://github.com/succinctlabs/sp1) zkVM, to help choose the hash for a Merkle map proven in SP1.

For x random data sets of a chosen size, it:

1. hashes every set natively, with timing
2. executes a guest that hashes every set with SP1's hash precompiles, and fails unless the result matches the native hash and the guest made exactly the expected precompile calls
3. proves that guest in one proof, with timing
4. verifies the proof, with timing, and checks that a wrong verifying key and changed public values are both rejected

This compares performance only. It says nothing about the security of each hash or its cost outside SP1.

## Requirements

- Rust stable (selected by `rust-toolchain.toml`)
- Docker, running: guests build inside `ghcr.io/succinctlabs/sp1:v6.10.0`, so no local SP1 toolchain is needed and ELFs and verifying keys are reproducible
- `protoc`
- RAM: proving used about 11 GiB of resident memory at 1,000 hashes and 17 to 18 GiB from 20,000 hashes up. On a 32 GiB machine shared with other work, swap use grew from about 130 to 500 million trace cells upward, and by 7 to 21 GiB for the largest proofs

The first build compiles about 500 crates and pulls the image (about 1.5 GB), so it takes a while with no output from the guest step. The image is public and is pulled without credentials, so you need no login, and a stored `ghcr.io` login cannot block it. If the image is removed later, for example by `docker system prune -a`, the next guest build pulls it again.

An editor running rust-analyzer starts the same guest build when it checks this crate. Concurrent builds take turns, so `cargo run` can wait silently while another one finishes.

## Usage

```sh
# Native timing, instructions, gas and precompile calls, without proving
cargo run --release -- -s 40,64,96 -n 1000 --execute-only

# Check everything runs, with fake proofs
SP1_PROVER=mock cargo run --release -- -s 64 -n 100

# Real proofs of the Merkle node variants
cargo run --release -- -s 64 -n 1000 -v baseline,sha256_node,keccak256_node,poseidon2_node
```

The last command makes 10 real proofs, 5 in each mode, because baseline runs twice. On a heavily loaded M2 Max that took about 11 minutes: 20 to 40 s per core proof and 50 to 95 s per compressed proof.

| Option | Meaning | Default |
|---|---|---|
| `-s, --data-size` | Bytes per set, comma separated. Accepts `1K`, `1M` | `64` |
| `-n, --count` | Number of sets x, comma separated. Also the hashes per proof | `1000` |
| `-v, --variants` | Variants to run, comma separated | all that apply |
| `-m, --modes` | `core`, `compressed` | both |
| `-r, --runs` | Proofs per variant and mode | `1` |
| `--execute-only` | Stop before proving. Rankings then use gas | off |
| `--seed` | Seed for the data and run order | random, printed |
| `--json` | Write a full report, updated after every measurement | off |

`SP1_PROVER` selects the prover: `cpu` by default, or `mock`. Groth16 and PLONK are left out because their wrap stage costs the same for every hash.

## Variants

Compare results only within a tier and data size.

| Variant | Tier | Sizes | Guest path |
|---|---|---|---|
| `baseline` | control | any | No hash. Reads every input word once, so subtracting it removes input handling |
| `sha256` | bytes | any | `sha2` crate, patched to use the SHA precompiles |
| `keccak256` | bytes | any | `tiny-keccak` crate, patched to use the Keccak precompile |
| `poseidon2` | bytes | any | SP1's `Poseidon2ByteHash`, a length-prefixed sponge taking 3 bytes per field element |
| `sha256_node` | node | 64 | Raw SHA precompile calls on native words. Standard SHA-256, with the padding block's schedule computed once |
| `keccak256_node` | node | 64 | One raw Keccak precompile call on native lanes. Standard Keccak-256 |
| `poseidon2_node` | node | 64 | One Poseidon2 precompile call on 16 field elements: `Trunc8(P(x))`, the form SP1 uses for its own Merkle commitments, which relies on every child being a hash output |
| `poseidon2_node_ff` | node | 64 | The same with the feed-forward from the [Poseidon2 paper](https://eprint.iacr.org/2023/323): `Trunc8(P(x) + x)` |

The node tier exists because the 64-byte node dominates a Merkle map, and a byte API overstates its cost. Each node variant is that hash's cheapest correct path, with children held in the hash's native words, so none pays for byte conversion a real tree can avoid.

The host never uses the precompile patches, so native timings use each crate's default fastest path: ARMv8 SHA2 and SHA3 instructions on Apple Silicon, while Keccak is software on x86-64. `Poseidon2ByteHash` only runs inside the zkVM, so the host has its own implementation over SP1's permutation, checked against the guest on every run.

## Reading the results

| Metric | What it is |
|---|---|
| Net | Per hash, minus `baseline`: roughly the cost of the hash path itself. Baseline's XOR and loop differ a little from each guest's, so net area is only good to a few hundred units. That matters only for Poseidon2, whose call is that small |
| Estimated real tree node | Each guest's own fitted area per hash, minus loading the child a real path already has in memory. It needs at least 2 values of `-n`. It does not use the baseline, so this is the trace area line to compare hashes on |
| Main trace area | Rows times main columns, summed over all chips and shards of a core proof, leaving out preprocessed columns. Deterministic, but only a rough guide to time. With several `-n` values it is fitted, which removes the fixed cost of extra shards |
| Shards | From core proofs. Keccak calls are deferred to separate precompile shards, so Keccak gets at least one extra shard at any count |
| Prove time | Wall clock. With several `-n` values it is fitted as `fixed + ms/hash x count`, which separates per-hash cost from fixed overhead. With one count, net time per hash includes any extra shard's fixed cost |
| Gas | SP1's pricing estimate. It leaves out global memory init and finalize for every touched word, and the memory rows deferred precompiles add around their calls, so it understates Keccak somewhat. Even so, on swap-free proofs a variant's full gas per hash tracked marginal prove time about as well as trace area did. Per-call gas does not: it leaves out the cost of loading input |
| Instructions | RISC-V instructions only. Rows inside precompile chips are not counted |

Flags mark prove times to distrust. They are not measured around setup, verification or native timing.

- `cold-only`: a single proof, so there is no warm timing
- `few-samples`: under 3 warm proofs
- `busy`: other processes used more than 1 core during a proof, or during the baseline it is compared with
- `swap`: swap use grew during a proof, or during that baseline
- `shards`: shard count differs from baseline
- `noise`: the difference from baseline is under twice the spread

In compressed mode the first proof with a client also compiles recursion programs, so the first proof is reported as cold and left out of rankings in both modes. Each variant gets a fresh client, and baseline runs both first and last at each point, so its drift shows in the spread.

## Choosing a hash for a Merkle map

An indexed Merkle map hashes a 64-byte node (two 32-byte children) on every level, a leaf such as 96 bytes (32-byte key, value and next key), and, if a commitment binds the map's size, a 40-byte root plus `u64` length. Use the node tier for the first and the bytes tier for the others.

```sh
# Deterministic trace area from core proofs, which does not depend on machine load
cargo run --release -- -s 40,64,96 -n 100,2000 -m core

# Timing, on an otherwise idle machine. Pick counts per family so each spans a few shards:
# Poseidon2 needs far higher counts than SHA-256 or Keccak to rise above the noise.
SP1_WORKER_NORMALIZE_PROGRAM_CACHE_SIZE=64 cargo run --release -- \
  -s 64 -v baseline,sha256_node,keccak256_node -n 2000,5000,10000 -r 4 --json results/sha-keccak.json
SP1_WORKER_NORMALIZE_PROGRAM_CACHE_SIZE=64 cargo run --release -- \
  -s 64 -v baseline,poseidon2_node,poseidon2_node_ff -n 50000,100000,200000 -r 4 --json results/poseidon2.json
```

The larger normalize cache keeps warm proofs warm when a run has many shard shapes.

In these runs both children of a node are fresh input, while on a real Merkle path one child is already in memory. Net figures remove all input cost, so their multiples overstate how far apart the hashes are in a real tree, where a node still pays about 4,200 area units to load its fresh child. Compare hashes on the estimated real tree node line, which accounts for this.

On macOS, SP1 runs guests with its portable executor rather than the x86-64 JIT, so proving times are not comparable with x86-64 Linux. CPU proving on macOS is also not covered by SP1's CI.
