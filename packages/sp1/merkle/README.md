# Merkle maps in SP1: adding and changing many records

Measures what it costs to prove a batch of records going into a real Merkle map inside SP1 6.10: an
indexed Merkle map and a Jellyfish tree, each with SHA-256 and Poseidon2. Where `../hash` measures
single hash calls, this measures whole state transitions: reading the witness, deriving keys and
values, verifying the old state, computing the new root and committing the result.

## Requirements

Docker, `protoc` and stable Rust. Guests build inside SP1's Docker image, so no local SP1 toolchain
is needed. The first build pulls the image and takes several minutes.

## Usage

```sh
# All tests but the two that run the real prover. The guests build once.
cargo test --release

# One point: a batch of 100 new records on a state of 100,000, with a core proof
cargo run --release -- one --arm imm_batch_poseidon2 --workload insert -k 100 --state 100000 --mode core

# A named list of measurements. Anything already in results/runs.jsonl is skipped.
cargo run --release -- run execute
cargo run --release -- run r1

# Tables from the recorded runs
cargo run --release -- report

# Forty consecutive blocks on one growing state, chaining their roots, without proofs
cargo run --release -- consecutive
```

`--mode execute` runs the guest with no proof and is the default for `one`. It gives exact gas,
instructions and hash calls, and checks the guest against the host.

| Round | What it measures | Rough time on a busy M2 Max with 32 GiB |
|---|---|---|
| `execute` | Every arm, batch size and option, without proofs | Twenty minutes |
| `deep` | Jellyfish arms but the Poseidon2 sponge, on 10 million records, without proofs. The state takes about 15 GB | Ten minutes |
| `r1` | Core proofs at 100 records, all arms but the two through the `sha2` crate, plus one insert through it at height 16 | Half an hour |
| `r1s` | The same for the two arms through the `sha2` crate | Twenty minutes |
| `r2a` | Core proofs at 1,000 records, batch arms and chained Poseidon2 | An hour |
| `r2b` | Core proofs at 1,000 records, stock arms but the Poseidon2 sponge, with only updates through `sha2`, and chained SHA-256 | About three hours |
| `r3` | Core proofs at 10,000 records, Poseidon2 batch arms and chained Poseidon2 updates | Two hours |
| `r3s` | Core proofs at 10,000 records, batch SHA-256 arms and chained Poseidon2 inserts | About four and a half hours |
| `r4` | Mixed batches and sequential keys | An hour |
| `r5` | Poseidon2 batch arms with a SHA-256 batch commitment | Ten minutes |
| `compressed` | Compressed proofs at a range of shard counts | One to two hours |

A proof is stopped when it takes three times what a time model predicts, up to four hours, or, on macOS,
when it needs more than 1.75 times the machine's memory. A stopped run counts as a result until a longer
limit of that kind applies. A failed run is tried again. So is any run whose guest has changed, or a
proof made under other prover settings.

## Memory

A proof's peak footprint counts what the system has compressed or swapped, so it can be far above resident
memory. At SP1's default settings it was about 11 GiB plus 43 GiB per billion cells, up to about 1.3 billion
cells. So on a 32 GiB Mac the stop above ends a default proof at about 1 billion cells, and every round from
`r1` up but `r5` has points near or above that. Without that stop, macOS killed proofs of about 1.5 billion cells and
more, and first ended background processes across the machine.

Fewer provers and shorter queues avoid that. On such a machine, set these before any round of proofs:

```sh
export SP1_WORKER_NUM_CORE_WORKERS=1 SP1_WORKER_CORE_BUFFER_SIZE=1
export SP1_WORKER_NUM_SPLICING_WORKERS=1 SP1_WORKER_SPLICING_BUFFER_SIZE=1
```

One core worker kept proofs of 2.6 to 13.2 billion cells to 34 to 36 GiB. Two kept most proofs of up to 3.2
billion cells to 47 to 56 GiB, and reached the stop on four. Time per cell did not measurably change between
these settings on a busy machine.

The settings are recorded with each run, and `report` shows the worker count. A proof made under other
settings is another row, so `run` proves it again.

## Maps and witnesses

Each arm is one guest.

| Arm | Map | Witness | Hash path |
|---|---|---|---|
| `imm_chained_sha2` | Indexed | One proof per record, Borsh | `sha2` crate |
| `imm_chained_sha256` | Indexed | One proof per record, Borsh | Raw SHA-256 calls |
| `imm_chained_poseidon2` | Indexed | One proof per record, Borsh | Field-native Poseidon2 |
| `imm_batch_sha256` | Indexed | One deduplicated witness, flat | Raw SHA-256 calls |
| `imm_batch_poseidon2` | Indexed | One deduplicated witness, flat | Field-native Poseidon2 |
| `jmt_stock_sha2` | Jellyfish | `jmt` update proofs, Borsh | `sha2` crate |
| `jmt_stock_sha256` | Jellyfish | `jmt` update proofs, Borsh | Raw SHA-256 behind the byte stream |
| `jmt_stock_poseidon2_sponge` | Jellyfish | `jmt` update proofs, Borsh | SP1's Poseidon2 byte sponge |
| `jmt_stock_poseidon2_node` | Jellyfish | `jmt` update proofs, Borsh | Field-native Poseidon2 nodes |
| `jmt_batch_sha256` | Jellyfish | One deduplicated witness, flat | Raw SHA-256 calls |
| `jmt_batch_poseidon2` | Jellyfish | One deduplicated witness, flat | Field-native Poseidon2 |

Roots are the same across a map's SHA-256 arms, and the same across its Poseidon2 node arms. The
indexed map's SHA-256 roots also match the crate it was adapted from, and the Jellyfish batch arms
reproduce `jmt`'s roots.

The maps are generic through three traits in `program/map/src/hash.rs`. A byte-stream trait would
lose the cheap paths, because a raw SHA-256 node shares one message schedule, and a Poseidon2 node is
one permutation of 16 field elements. So the traits work at the level of tree operations.

The indexed map is adapted from `crates/indexed-merkle-map` in
[SilvanaOne/silvana](https://github.com/SilvanaOne/silvana) (Apache-2.0). The module notice in
`program/map/src/imm/mod.rs` lists what changed.

## Records and workloads

A record is a key preimage and a payload. A guest derives the key and value digests with the hash
under test, verifies the old state, applies the change and computes the new root itself. So every
write comes from a verified read, and the host only supplies a witness.

| Workload | Records |
|---|---|
| `insert` | New records with hashed keys |
| `update` | Changes to existing records, each key once, spread uniformly |
| `mixed` | 80% changes with Zipf hot keys, so some keys repeat, and 20% new records |
| `append` | New records with sequential keys, on a state of sequential keys, like a journal. The indexed map uses the sequence number as its key. Jellyfish hashes it, so there this is an insert with a short key |

The defaults are an 84-byte key preimage and a 24-byte payload (a `u128` amount and a `u64` nonce).
These are assumptions, as are the proportions in `mixed`. Change them with `--key-size` and
`--value-size`.

Every guest commits the same 152 bytes: the versions, the settings the witness chose (flags, record
sizes and any measurement stop point), the tree height and lengths where the map has them, the old
and new roots, and a commitment to the batch's records in order. A verifier has to require a zero stop
point and pin the height, flags and sizes it expects: a guest can only commit them.

The batch commitment uses the map's own hash. With `--external-batch-hash`, Poseidon2 arms instead
commit the same SHA-256 chain as the SHA-256 arms, which anyone can recompute from the records.

## What is checked

Before a proof starts, each run executes the guest and fails unless the exit code is 0, the public
values equal the host's, and the counts of each hash precompile equal what the same engine counted on
the host. That last check catches a precompile patch that was silently dropped. Each proof is then
verified, and verification must reject changed public values and another program's verifying key.

`cargo test --release` runs:

- the adapted map against the original crate, on edge keys and random operations
- the indexed map against a plain ordered map, and each map's engines against each other, including
  sub-batches on the indexed map and consecutive batches on both
- the Jellyfish batch arms and `jmt` against an independent definition of the tree
- tampered witnesses, which every engine must reject, on the host and in the guests

`cargo test --release -- --ignored` runs only the tests of the real prover, which takes about eleven minutes: the
guests that call raw SHA-256 must prove and verify, and a non-canonical Poseidon2 element must not
give a proof that verifies. Execution alone cannot show either.

## Reading the results

| Column | Meaning |
|---|---|
| gas/rec | SP1's gas estimate per record, deterministic |
| instr/rec | RISC-V instructions per record, deterministic. Precompile rows are extra |
| sha c/rec, p2/rec | SHA-256 compressions and Poseidon2 permutations per record, deterministic |
| wit B/rec | Witness bytes per record. Each 8-byte word costs about 1,044 trace cells |
| native ms | The same engine on the host, for the whole batch, including all checks |
| prove s | Wall clock of the proof, on a machine that may be busy |
| cells/rec | Main trace area per record, from a core proof, deterministic |
| us/gas | Proving time per unit of gas, on a machine that may be busy |
| RSS | Peak resident memory. It falls when the system compresses or swaps the process, so it is not what a proof needs |
| peak | Peak physical footprint, including what was compressed or swapped. This is what a proof needs (macOS only) |
| swap+, other cores | What the machine was doing during the proof. Distrust times where these are high |
| settings | What sets a run apart from the standard one of its arm: core workers, stop point, blocks, sizes, options |

Compare across arms and hashes on cells. Gas compares runs within one arm: cells per gas runs from 48 to
72 between arms from 100 records up, so a run that was only executed needs its arm's ratio from a proved run. Small time
differences are noise on a busy machine. Every proof has a fixed floor of about 13 s for core and 45 s
for compressed, so per-record times mean little until a batch is well above it.

The stop points give the cost of each phase by difference. On chained, stock and Jellyfish batch arms,
`stop=derive` is after deriving keys and the batch commitment, and `stop=decode` is after that and
decoding the witness. On `imm_batch_*` the order differs: `stop=decode` comes first and has done almost
nothing, and `stop=derive` also hashes every touched leaf. So there, deriving and old leaves are
`stop=derive` minus `stop=decode`, and the walk is the full run minus `stop=derive`. Decoding also means
different things by format: on stock arms it includes the sort, the Borsh decode, the guard and hashing
old values, and on Jellyfish batch arms it is only the sort.

The height rows on batch arms mix height and fill. Batches share more of each path on a small state, so
compare a small state at heights 16, 21 and 26 to see the height alone.

Delete `data/witness` after changing the code that builds records or witnesses, because witnesses are
reused by name. A recorded run only counts for the same guest, witness and prover settings.

## A trap in SP1 6.10

SP1's prover only includes memory that a CPU load or store touched, or that is in the program's image.
A precompile's own reads and writes do not count. So a message schedule on stack the CPU never touched,
with words 16 to 63 left for `sha256_extend` to write, executes normally with exact counts and then gives
a proof that fails verification with `global cumulative sum is not zero`. The guests that call raw SHA-256
build every schedule in one static, which is in the image and costs nothing per block. Zeroing a schedule per block
also works, but costs 48 more stores.

## Limits

- Poseidon2 arms rank proving cost only. Nothing outside the proof can verify a Poseidon2 root here.
- A Poseidon2 node has no domain tag, and its permutation can be inverted, so anyone can open any digest
  as a node with unchosen children. Leaves are safe against this: a Jellyfish leaf is the second half of
  the last state, and the indexed map's depth is fixed by a height the verifier has to pin.
- Deletes are not implemented. The Poseidon2 node path would need a node tag or a feed-forward before
  it could safely support them.
- `append` prices sequential keys. It does not enforce them: a verifier checks the records it commits to.
- Reads without writes, real contract execution, and Groth16 or PLONK wrapping are not measured.
- macOS uses SP1's portable executor, so times are not comparable with x86-64 Linux.
