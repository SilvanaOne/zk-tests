# Indexed Merkle Map on Canton Network

A Daml smart contract implementing cryptographically verified indexed merkle map operations with witness-based proofs for privacy-preserving state transitions on Canton Network.

## Overview

This contract enables privacy-preserving key-value storage using an **Indexed Merkle Map** - a sorted merkle tree where operations (insert, update, membership/non-membership proofs) are verified via cryptographic witnesses without revealing the entire tree structure.

**Key Features:**

- ✅ **Witness-based verification**: Verify insertions and updates without reconstructing the full map
- ✅ **Membership & non-membership proofs**: Prove key existence or non-existence with cryptographic guarantees
- ✅ **Fixed-height tree (32 levels)**: Supports up to 2³¹ = 2,147,483,648 elements
- ✅ **Defense-in-depth validation**: 9+ verification layers including hex format, bounds checks, path consistency
- ✅ **Timestamp tracking**: Ledger Effective Time (LET) captured for audit trails
- ✅ **SHA256-based hashing**: Uses Canton's alpha `DA.Crypto.Text` library

## Architecture

### Data Structures

#### Core Types

```daml
-- Leaf in the indexed merkle tree (sorted by key)
data Leaf = Leaf with
    key : Text        -- 64-char hex (32 bytes)
    value : Text      -- 64-char hex (32 bytes)
    nextKey : Text    -- Next key in sorted order (for linked list structure)
    index : Int       -- Leaf position in tree (0-indexed)

-- Merkle proof path from leaf to root
data MerkleProof = MerkleProof with
    siblings : [Text]      -- 31 sibling hashes (fixed height 32)
    pathIndices : [Bool]   -- Path directions (false=left, true=right)
```

#### Witness Types

**InsertWitness** - Proves a new key-value pair can be inserted:

- Contains old/new roots, key-value, tree metadata
- Includes non-membership proof (low leaf) and updated tree structure
- Verifies 8 constraints: append-only, path consistency, key ordering, root computation, etc.

**UpdateWitness** - Proves an existing key's value can be updated:

- Contains old/new roots, key, old/new values
- Includes membership proof for existing key
- Verifies 3 constraints: membership, leaf structure, root computation

#### Proof Types

**MembershipProof** - Proves a key-value exists in the map:

- Used by `VerifyInclusion` non-consuming choice
- Verifies leaf matches key-value and merkle path computes to root

**NonMembershipProof** - Proves a key does NOT exist in the map:

- Uses "low leaf" concept: finds largest key < query key
- Proves query key falls in gap: `lowLeaf.key < queryKey < lowLeaf.nextKey`
- Used by `VerifyExclusion` non-consuming choice

### Contract State

```daml
template Hash
  with
    owner : Party
    add_result : Int               -- Simple addition result (legacy)
    keccak_result : Optional Text  -- Keccak256 hash result (legacy)
    sha256_result : Optional Text  -- SHA256 hash result (legacy)
    root : Optional Text           -- Current merkle map root (hex-encoded)
    root_time : Optional Time      -- Timestamp when root was last updated
```

### Choices

#### Merkle Map Operations

**AddMapElement** - Insert new key-value pair

```daml
choice AddMapElement : HashId
  with witness : InsertWitness
  controller owner
```

- Verifies insert witness with 8 cryptographic constraints
- Updates root and timestamp
- **Defense-in-depth**: Hex validation, proof bounds, index checks, LCA anchor verification

**UpdateMapElement** - Update existing key's value

```daml
choice UpdateMapElement : HashId
  with witness : UpdateWitness
  controller owner
```

- Verifies update witness with 3 constraints
- Updates root and timestamp
- Validates membership proof, leaf structure, root computation

**VerifyInclusion** - Non-consuming proof of membership

```daml
nonconsuming choice VerifyInclusion : Bool
  with
    proof : MembershipProof
    requester : Party
  controller requester
```

- Verifies a key-value exists in the current map state
- Does not modify contract (non-consuming)
- Returns `True` if proof valid

**VerifyExclusion** - Non-consuming proof of non-membership

```daml
nonconsuming choice VerifyExclusion : Bool
  with
    proof : NonMembershipProof
    requester : Party
  controller requester
```

- Verifies a key does NOT exist in the current map state
- Uses low leaf to prove gap in sorted keys
- Does not modify contract (non-consuming)

#### Add and Hash Operations

**Add** - Sum a list of integers
**Keccak** - Compute Keccak256 hash of concatenated hex strings
**Sha256** - Compute SHA256 hash of concatenated hex strings
**Sha256n** - Iteratively hash n times: `hash_i+1 = sha256(hash_i || args)`

## Verification Logic

### Insert Witness Verification (8 Constraints)

```daml
verifyInsertWitness : InsertWitness -> Bool
```

**Defense-in-depth layers:**

1. **Hex format validation**: All 32-byte fields are valid lowercase hex
2. **Proof structure**: Exactly 31 siblings, matching path indices
3. **Tree capacity**: Tree length within 2³¹ limit
4. **Index bounds**: All indices within valid ranges
5. **Append-only**: `newLeafIndex == treeLength`
6. **Path consistency**: `pathIndicesToIndex(pathIndices) == leaf.index`
7. **Non-membership**: Query key doesn't exist in old tree (via low leaf proof)
8. **Key ordering**: `lowLeaf.key < newKey < lowLeaf.nextKey`
9. **Leaf structure**: Updated low leaf and new leaf have correct fields
10. **Root computation**: New root matches computed merkle root
11. **LCA anchor check**: Updated low leaf path links to new leaf proof (prevents unrelated proof attacks)

### Update Witness Verification (3 Constraints)

```daml
verifyUpdateWitness : UpdateWitness -> Bool
```

1. **Membership proof**: Key exists with old value in old tree
2. **Leaf structure**: Updated leaf has correct key, new value, same next_key and index
3. **Root computation**: New root matches computed merkle root with updated leaf

### Membership Proof Verification

```daml
verifyMembershipProof : MembershipProof -> Bool
```

- Validates hex format, proof structure (31 siblings), bounds
- Verifies leaf contains query key-value
- Computes merkle root and compares to provided root

### Non-Membership Proof Verification

```daml
verifyNonMembershipProof : NonMembershipProof -> Bool
```

- **Key ordering check**: `lowLeaf.key < queryKey`
- **Next key check**: `queryKey < lowLeaf.nextKey` (or `nextKey == 0` if rightmost)
- Verifies low leaf exists in tree via merkle proof
- Proves query key falls in gap between consecutive keys

## Cryptographic Primitives

### Hash Functions

```daml
-- Hash a leaf: sha256(key || value || next_key)
hashLeaf : Leaf -> Text

-- Hash two nodes: sha256(left || right)
hashPair : Text -> Text -> Text

-- Compute merkle root from leaf and proof
computeRoot : Text -> MerkleProof -> Text

-- Combine internal root with tree length: sha256(internal_root || length_bytes)
combineRootWithLength : Text -> Int -> Text
```

### Path Operations

```daml
-- Convert merkle path to leaf index (little-endian bit representation)
pathIndicesToIndex : [Bool] -> Int

-- Find level where two paths diverge (Lowest Common Ancestor)
findLCALevel : [Bool] -> [Bool] -> Int

-- Hash from leaf up to specific level
hashUpToLevel : Text -> [Text] -> [Bool] -> Int -> Text
```

## Usage Example

### 1. Create Hash Contract

```bash
# Initialize with empty root (or None)
curl -X POST "$API_URL/v2/commands/submit-and-wait" \
  -H "Authorization: Bearer $JWT" \
  -d '{
    "commands": [{
      "CreateCommand": {
        "templateId": "PACKAGE_ID:Hash:Hash",
        "createArgument": {
          "owner": "PARTY_ID",
          "add_result": "0",
          "keccak_result": null,
          "sha256_result": null,
          "root": "INITIAL_ROOT_HEX",
          "root_time": null
        }
      }
    }]
  }'
```

### 2. Insert Key-Value Pair

```bash
# Generate InsertWitness using Rust client
cargo run -- add-map-element 44 900

# This performs 3 phases:
# Phase 1: INSERT (exercises AddMapElement with witness)
# Phase 2: VERIFY INCLUSION (exercises VerifyInclusion)
# Phase 3: VERIFY EXCLUSION of KEY=1000 (exercises VerifyExclusion)
```

### 3. Update Existing Key

```bash
# Generate UpdateWitness using Rust client
cargo run -- update-map-element 44 950 1000
```

### 4. Query Membership (Non-Consuming)

```bash
# Exercises VerifyInclusion without consuming contract
curl -X POST "$API_URL/v2/commands/submit-and-wait" \
  -d '{
    "commands": [{
      "ExerciseCommand": {
        "templateId": "PACKAGE_ID:Hash:Hash",
        "contractId": "CONTRACT_ID",
        "choice": "VerifyInclusion",
        "choiceArgument": {
          "proof": { /* MembershipProof JSON */ },
          "requester": "PARTY_ID"
        }
      }
    }]
  }'
```

## Security Properties

### Fixed Tree Height (Audit Compliance)

All proofs enforce **exactly 31 siblings** (height 32 tree):

```daml
proofBoundsCheck = proofSiblingsLen == 31
```

This prevents:

- Variable-height attacks
- Proof length manipulation
- Inconsistent tree structures

### Defense-in-Depth Validation

Every verification function includes:

1. **Hex format validation**: Prevents malformed input
2. **Proof structure checks**: Siblings match path indices
3. **Tree capacity checks**: Prevent overflow beyond 2³¹ leaves
4. **Index bounds checks**: All indices within valid ranges
5. **Path-index consistency**: Computed index matches claimed index
6. **Cryptographic verification**: Merkle proofs compute to expected root

### LCA Anchor Check (Insert Only)

Prevents using unrelated proofs from different trees:

```daml
-- Hash updatedLowLeaf up to LCA level
anchorFromUpdatedLowLeaf = hashUpToLevel (hashLeaf witness.updatedLowLeaf) ...

-- Verify it matches sibling at LCA in new leaf proof
lcaAnchorCheck = anchorFromUpdatedLowLeaf == expectedSiblingAtLCA
```

This links the low leaf update to the new leaf insertion, ensuring both proofs come from the same tree state.

### No-Op Update Prevention (Optional)

While not enforced, the Rust client prevents updating a key to its current value (audit recommendation).

## Technical Details

### Tree Structure

- **Type**: Indexed Merkle Map (sorted key-value store with merkle tree)
- **Height**: Fixed at 32 levels (31 proof siblings)
- **Capacity**: 2³¹ = 2,147,483,648 leaves maximum
- **Hash function**: SHA256 for all operations
- **Key/value format**: 32-byte fields encoded as 64-char lowercase hex
- **Leaf structure**: `sha256(key || value || next_key)`
- **Node structure**: `sha256(left_child || right_child)`
- **Root structure**: `sha256(internal_root || tree_length_8_bytes)`

### Ledger Time

All state-modifying choices capture **Ledger Effective Time (LET)**:

```daml
currentTime <- getTime  -- Returns Time with microsecond resolution
```

**Properties:**

- Monotonically increasing across transactions
- Same value for all `getTime` calls within a transaction
- Deterministic and replayable (for privacy verification)
- Closely matches real time for live transactions

### Witness Generation

Witnesses are generated **off-chain** using the Rust `indexed-merkle-map` crate:

- Path: `/Users/mike/Documents/Silvana/silvana/crates/indexed-merkle-map`
- Rust client: `/Users/mike/Documents/Silvana/zk-tests/packages/canton/hash/rust`

The Rust client:

1. Maintains in-memory indexed merkle map
2. Generates cryptographic witnesses for operations
3. Submits witnesses to Canton via Ledger API v2
4. Verifies operations using non-consuming choices

## Files

- `daml/Hash.daml` - Main contract implementation (this file)
- `daml.yaml` - Project configuration
- `Makefile` - Build, upload, and inspection commands
- `.env` - Environment variables (API URLs, party IDs, package ID)
- `rust/` - Rust client for witness generation and Canton API interaction

## Building and Deployment

```bash
# Build DAR
make build

# Upload to Canton Network
make upload

# Get package ID
make inspect

# List parties
make parties

# Query updates
make updates OFFSET=15000
```

## Dependencies

- `daml-prim`
- `daml-stdlib`
- `DA.Crypto.Text` (alpha - for `sha256` and `keccak256`)
