# Indexed Merkle Map on Canton Network

A Daml smart contract implementing cryptographically verified indexed merkle map operations with witness-based proofs for privacy-preserving state transitions on Canton Network.

## Overview

This contract enables privacy-preserving key-value storage using an **Indexed Merkle Map** - a sorted merkle tree where operations (insert, update, membership/non-membership proofs) are verified via cryptographic witnesses without revealing the entire tree structure.

**Key Features:**

- ✅ **Cross-node observers**: Full support for observers on different Canton participant nodes
- ✅ **Propose-accept pattern**: Proper authorization for cross-node contract creation
- ✅ **Witness-based verification**: Verify insertions and updates without reconstructing the full map
- ✅ **Membership & non-membership proofs**: Prove key existence or non-existence with cryptographic guarantees
- ✅ **Fixed-height tree (32 levels)**: Supports up to 2³¹ = 2,147,483,648 elements
- ✅ **Defense-in-depth validation**: 9+ verification layers including hex format, bounds checks, path consistency
- ✅ **Timestamp tracking**: Ledger Effective Time (LET) captured for audit trails
- ✅ **SHA256-based hashing**: Uses Canton's alpha `DA.Crypto.Text` library
- ✅ **Transaction visibility**: Full transaction details with updateIds for all operations
- ✅ **Archive support**: Complete lifecycle including cross-node archive visibility

## Architecture

### Cross-Node Design

This implementation demonstrates a **multi-participant Canton setup** with proper cross-node observer patterns:

**Topology:**

```
┌─────────────────────┐         ┌─────────────────────┐
│   User Node         │         │   Provider Node     │
│   (Participant 1)   │         │   (Participant 2)   │
│                     │         │                     │
│  app_user (owner)   │◄───────►│  app_provider       │
│    - signatory      │         │    - observer       │
│    - initiates ops  │         │    - cross-node     │
└─────────────────────┘         └─────────────────────┘
          │                               │
          └───────────┬───────────────────┘
                      ▼
              ┌──────────────┐
              │ Synchronizer │
              │ (Domain)     │
              └──────────────┘
```

**Key Design Patterns:**

1. **Propose-Accept for Creation**: Uses `HashRequest` → `Accept` pattern to properly authorize cross-node contract creation
2. **SynchronizerId Parameter**: All commands include `synchronizerId` to route transactions through the correct domain
3. **Package Vetting**: Both nodes must have the package vetted for the respective parties
4. **Cross-Node Visibility**: All operations (create, update, verify, archive) are visible to both parties
5. **Dual Fetch**: Critical operations fetch updates from both nodes to verify synchronization

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

### Contract Templates

#### HashRequest (Propose-Accept Pattern)

```daml
template HashRequest
  with
    owner : Party
    provider : Party
    id : Text
    add_result : Int
    keccak_result : Optional Text
    sha256_result : Optional Text
    root : Text
  where
    signatory owner
    observer provider

    choice Accept : ContractId Hash
      controller provider

    choice Decline : ()
      controller provider

    choice Withdraw : ()
      controller owner
```

**Purpose**: Enables cross-node contract creation where the `provider` (observer) is hosted on a different Canton participant node than the `owner`. This implements the proper propose-accept pattern for cross-node authorization.

#### Hash (Main Contract)

```daml
template Hash
  with
    owner : Party
    provider : Party               -- Provider party (maintainer, cross-node observer)
    id : Text                      -- Unique identifier
    add_result : Int               -- Simple addition result (legacy)
    keccak_result : Optional Text  -- Keccak256 hash result (legacy)
    sha256_result : Optional Text  -- SHA256 hash result (legacy)
    root : Text                    -- Current merkle map root (hex-encoded)
    root_time : Optional Time      -- Timestamp when root was last updated
  where
    signatory owner
    observer provider              -- Cross-node observer
```

### Choices

#### Merkle Map Operations (via IndexedMerkleMap Interface)

The `Hash` template implements the `IndexedMerkleMap` interface, providing standardized merkle map operations:

**AddMapElement** - Insert new key-value pair

```daml
choice AddMapElement : ContractId IndexedMerkleMap
  with witness : InsertWitness
  controller owner
```

- Verifies insert witness with 8 cryptographic constraints
- Updates root and timestamp
- Returns interface contract ID (consuming choice - creates new contract)
- **Defense-in-depth**: Hex validation, proof bounds, index checks, LCA anchor verification

**UpdateMapElement** - Update existing key's value

```daml
choice UpdateMapElement : ContractId IndexedMerkleMap
  with witness : UpdateWitness
  controller owner
```

- Verifies update witness with 3 constraints
- Updates root and timestamp
- Returns interface contract ID (consuming choice - creates new contract)
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

**Archive** - Archive the contract (built-in choice)

```daml
choice Archive : ()
  controller owner
```

- Standard DAML choice available on all templates
- Archives (consumes) the contract, removing it from active contract set
- Cross-node observers (provider) can see the archive event
- No return value (consuming choice)

#### Hash-Specific Operations (Template Choices)

**Add** - Sum a list of integers
**Keccak** - Compute Keccak256 hash of concatenated hex strings
**Sha256** - Compute SHA256 hash of concatenated hex strings
**Sha256n** - Iteratively hash n times: `hash_i+1 = sha256(hash_i || args)`

Note: These are direct template choices, not part of the IndexedMerkleMap interface.

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

### Cross-Node Setup

This implementation supports **cross-node observers** where:

- `owner` (app_user) is hosted on one Canton participant node
- `provider` (app_provider) is hosted on a different Canton participant node
- Both parties can observe all contract operations and events

**Environment Configuration**:

```bash
# User node
APP_USER_API_URL=http://localhost:7575/api/json-api/
APP_USER_JWT=...
PARTY_APP_USER=app_user_localnet-localparty-1::122...

# Provider node (different participant)
APP_PROVIDER_API_URL=http://localhost:7576/api/json-api/
APP_PROVIDER_JWT=...
PARTY_APP_PROVIDER=app_provider_localnet-localparty-1::122...

# Synchronizer (domain) for cross-node transactions
SYNCHRONIZER_ID=global-domain::122075d227a0...
```

### Complete Workflow (add-map-element)

The Rust client demonstrates the full cross-node lifecycle:

```bash
# Execute complete workflow: Create → Update → Verify → Archive
cargo run -- add-map-element 1001 30
```

**Workflow Phases**:

**Phase 1: Create Contract (Propose-Accept Pattern)**

- User creates `HashRequest` (signatory: owner, observer: provider)
- Provider accepts request via `Accept` choice
- Creates final `Hash` contract visible to both nodes
- Includes `synchronizerId` for cross-node coordination

**Phase 2: Insert Key-Value (AddMapElement)**

- Generates cryptographic `InsertWitness` off-chain
- Submits via interface choice `AddMapElement`
- Updates merkle map root with ZK proof verification
- Fetches update from **both** user and provider nodes

**Phase 3: Verify Inclusion**

- Exercises non-consuming `VerifyInclusion` choice
- Proves key-value exists in map via membership proof
- Contract remains active (non-consuming)

**Phase 4: Verify Exclusion**

- Exercises non-consuming `VerifyExclusion` choice
- Proves query key (1000) does NOT exist in map
- Uses "low leaf" proof to show gap in sorted keys

**Phase 5: Atomic Archive and Recreate**

- **Demonstrates atomic multi-command transactions in Canton**
- Single transaction contains TWO commands:
  1. `ExerciseCommand` - Archive old contract
  2. `CreateCommand` - Create new Hash contract with identical fields
- Both operations succeed or fail together (atomicity guarantee)
- New contract has same owner, provider, id, root, and all other fields
- Fetches transaction from **both** user and provider nodes
- Both nodes observe in the same transaction:
  - `ArchivedEvent` for old contract
  - `CreatedEvent` for new contract
- Confirms atomic cross-node transaction visibility
- No temporal gap between archive and create (happens atomically)

### Update Existing Key

```bash
# Generate UpdateWitness and update existing key
cargo run -- update-map-element 1001 950 2000
```

### Hash Operations

```bash
# Simple addition
cargo run -- add 10 20 30

# Keccak256 hash
cargo run -- keccak "deadbeef" "cafebabe"

# SHA256 hash
cargo run -- sha256 "deadbeef"

# Iterative SHA256 (n times)
cargo run -- sha256n "deadbeef" 5
```

## Security Properties

### Fixed Tree Height

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

### DAML Files

- `daml/Hash.daml` - Main `Hash` template with IndexedMerkleMap interface implementation
- `daml/HashRequest.daml` - Propose-accept pattern for cross-node contract creation
- `daml/Silvana.daml` - IndexedMerkleMap interface and verification functions
- `daml.yaml` - Project configuration (package name: hash-v8)

### Rust Client

- `rust/src/main.rs` - CLI entry point
- `rust/src/contract.rs` - Canton API interaction functions
  - `extract_hash_fields()` - Extract Hash contract fields from update JSON
  - `create_hash_contract()` - Creates HashRequest
  - `accept_hash_request()` - Provider accepts request
  - `create_and_accept_hash_contract()` - Full propose-accept workflow
  - `exercise_choice()` - Generic choice execution
  - `archive_and_recreate_hash_contract()` - Atomic archive + create transaction
  - `get_update()` - Fetch transaction details by updateId
- `rust/src/addmapelement.rs` - Complete add-map-element workflow (5 phases)
- `rust/src/updatemapelement.rs` - Update existing key workflow
- `rust/src/add.rs`, `keccak.rs`, `sha256.rs`, `sha256n.rs` - Hash operations

### Configuration

- `.env` - Environment variables (API URLs, JWTs, party IDs, package ID, synchronizer ID)
- `Makefile` - Build, upload, vetting, and inspection commands

## Building and Deployment

### DAML Package

```bash
# Build DAR
make build

# Upload to both nodes
make upload-user      # Upload to user node
make upload-provider  # Upload to provider node

# Get package ID
make inspect

# Check vetting status (important for cross-node)
make vetting-status-user
make vetting-status-provider

# List parties
make parties-user      # Parties on user node
make parties-provider  # Parties on provider node

# Query updates
make updates OFFSET=15000
```

### Cross-Node Package Vetting

**Important**: For cross-node transactions, packages must be vetted on both participant nodes:

1. Auto-vetting: Some nodes (e.g., with Splice validator backend) automatically vet packages on upload
2. Manual vetting: Other nodes require explicit vetting via Canton console:
   ```scala
   `app-provider`.dars.vetting.enable("PACKAGE_ID")
   ```

Use `make vetting-status-provider-only` to verify the provider party has the package vetted.

### Rust Client

```bash
cd rust
cargo build
cargo run -- add-map-element 1001 30
```

## Dependencies

- `daml-prim`
- `daml-stdlib`
- `DA.Crypto.Text` (alpha - for `sha256` and `keccak256`)
