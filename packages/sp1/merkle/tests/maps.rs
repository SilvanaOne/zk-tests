//! The maps on the host: model checks against a plain ordered map, agreement between engines, and
//! the witnesses each engine must reject. These run the same engine code a guest runs.

use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Once,
};

use jmt::{JellyfishMerkleTree, KeyHash, SimpleHasher};
use merkle_bench::{
    arm::Arm,
    imm_state::{ImmState, batch_witness, batch_witness_of, chained_witness, chained_witness_of},
    jmt_state::{self, State},
    measure::{aligned, as_bytes},
    point::{Point, RunMode, Workload},
    software::Native,
    workload::{self, Rec},
};
use merkle_map::{
    hash::{Digest, Hasher, ImmHasher, JmtHasher, Poseidon2, Sha2Crate, Sha256Raw},
    imm::{
        batch::{BlockHeader, TouchedLeaf},
        host::IndexedMerkleMap,
        provable::ProvableIndexedMerkleMap,
        types::{IndexedMerkleMapError, InsertWitness},
    },
    jellyfish::hashers::{PoseidonNode, PoseidonSponge, Sha2Counted, Sha256Stream},
    record::{HEADER_BYTES, Header, OP_INSERT, OP_UPDATE, PublicValues},
};
use sha2::Digest as _;

type Sha = Sha256Raw<Native>;
type Pos = Poseidon2<Native>;

fn point(arm: Arm, workload: Workload, k: u32, state: u64, height: u32, sub_batches: u32) -> Point {
    Point {
        arm,
        workload,
        k,
        sub_batches,
        state,
        height: if arm.is_jmt() { 0 } else { height },
        key_size: if workload == Workload::Append { 8 } else { 84 },
        value_size: 24,
        read_proofs: false,
        external_batch_hash: false,
        stop_after: 0,
        mode: RunMode::Execute,
        seed: 7,
    }
}

fn run(arm: Arm, witness: &[u8]) -> PublicValues {
    arm.native(as_bytes(&aligned(witness)))
}

thread_local! {
    static EXPECTING_PANIC: Cell<bool> = const { Cell::new(false) };
}

/// Runs something that may panic, without printing the panics a test expects. Others still print.
/// Gives the panic's message, if there was one.
fn panic_message(check: impl FnOnce()) -> Option<String> {
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if !EXPECTING_PANIC.get() {
                default(info);
            }
        }));
    });
    EXPECTING_PANIC.set(true);
    let outcome = catch_unwind(AssertUnwindSafe(check));
    EXPECTING_PANIC.set(false);
    outcome.err().map(|payload| {
        payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|text| text.to_string()))
            .unwrap_or_default()
    })
}

fn panics(check: impl FnOnce()) -> bool {
    panic_message(check).is_some()
}

/// Whether an engine rejects a witness. Rejection is a panic, which is a failed guest.
fn rejects(arm: Arm, witness: &[u8]) -> bool {
    let words = aligned(witness);
    panics(|| {
        arm.native(as_bytes(&words));
    })
}

/// Requires an engine to reject a witness for the expected reason, so a test cannot pass by accident
/// on some earlier check.
fn assert_rejected(arm: Arm, witness: &[u8], reason: &str) {
    let words = aligned(witness);
    let message = panic_message(|| {
        arm.native(as_bytes(&words));
    })
    .unwrap_or_else(|| panic!("{} accepted a witness: {reason}", arm.name()));
    assert!(
        message.contains(reason),
        "{} rejected with {message:?}, not {reason:?}",
        arm.name()
    );
}

/// A witness split back into what `assemble` takes.
fn regions(witness: &[u8]) -> (Header, Vec<u8>, Vec<u8>, Vec<u8>) {
    let words = aligned(witness);
    // SAFETY: the buffer starts with a header.
    let header: Header = unsafe { as_bytes(&words).as_ptr().cast::<Header>().read() };
    let records_end = HEADER_BYTES + header.records_bytes as usize;
    let olds_end = records_end + header.olds_bytes as usize;
    (
        header,
        witness[HEADER_BYTES..records_end].to_vec(),
        witness[records_end..olds_end].to_vec(),
        witness[olds_end..].to_vec(),
    )
}

fn digest(seed: u64) -> Digest {
    // Stays below the KoalaBear modulus in every limb, for both hashes.
    Digest(core::array::from_fn(|i| {
        ((seed
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .rotate_left(i as u32 * 7)
            >> 8) as u32)
            & 0x00ff_ffff
    }))
}

fn sha256_hex_words(bytes: &[u8]) -> Digest {
    let digest: [u8; 32] = sha2::Sha256::digest(bytes).into();
    Sha::import(&digest)
}

// ---------------------------------------------------------------------------------------------
// Hash profiles
// ---------------------------------------------------------------------------------------------

#[test]
fn raw_sha256_is_standard_sha256() {
    for length in [0, 1, 40, 55, 56, 63, 64, 65, 77, 80, 96, 119, 120, 1024] {
        let data: Vec<u8> = (0..length).map(|i| i as u8).collect();
        assert_eq!(
            Sha::bytes(&data),
            sha256_hex_words(&data),
            "length {length}"
        );
        assert_eq!(
            Sha::bytes(&data),
            Sha2Crate::<Native>::bytes(&data),
            "length {length}"
        );
    }
    let (a, b, c) = (digest(1), digest(2), digest(3));
    let cat = |parts: &[&Digest]| {
        let mut bytes = Vec::new();
        for part in parts {
            bytes.extend_from_slice(&Sha::export(part));
        }
        bytes
    };
    assert_eq!(
        <Sha as ImmHasher>::node(&a, &b),
        sha256_hex_words(&cat(&[&a, &b]))
    );
    assert_eq!(
        <Sha as ImmHasher>::leaf(&a, &b, &c),
        sha256_hex_words(&cat(&[&a, &b, &c]))
    );
    let mut bound = Sha::export(&a).to_vec();
    bound.extend_from_slice(&0x0102_0304_0506_0708u64.to_be_bytes());
    assert_eq!(
        Sha::bind(&a, 0x0102_0304_0506_0708),
        sha256_hex_words(&bound)
    );
    let mut internal = b"JMT::IntrnalNode".to_vec();
    internal.extend_from_slice(&cat(&[&a, &b]));
    assert_eq!(Sha::internal(&a, &b), sha256_hex_words(&internal));
    let mut leaf = b"JMT::LeafNode".to_vec();
    leaf.extend_from_slice(&cat(&[&a, &b]));
    assert_eq!(<Sha as JmtHasher>::leaf(&a, &b), sha256_hex_words(&leaf));
}

fn stream_hash<S: SimpleHasher>(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = S::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize()
}

#[test]
fn stream_hashers_match_their_profiles() {
    let (a, b) = (digest(1), digest(2));
    let (sa, sb) = (Sha::export(&a), Sha::export(&b));
    for parts in [
        vec![b"JMT::IntrnalNode".as_slice(), &sa, &sb],
        vec![b"JMT::LeafNode".as_slice(), &sa, &sb],
        vec![b"some value".as_slice()],
    ] {
        let joined = parts.concat();
        assert_eq!(
            stream_hash::<Sha256Stream<Native>>(&parts),
            Sha::export(&Sha::bytes(&joined))
        );
        assert_eq!(
            stream_hash::<Sha2Counted<Native>>(&parts),
            Sha::export(&Sha::bytes(&joined))
        );
        assert_eq!(
            stream_hash::<sha2::Sha256>(&parts),
            Sha::export(&Sha::bytes(&joined))
        );
        assert_eq!(
            stream_hash::<PoseidonSponge<Native>>(&parts),
            Pos::export(&Pos::bytes(&joined))
        );
    }
}

#[test]
fn poseidon_node_hasher_only_takes_the_exact_node_shape() {
    use jmt::proof::{INTERNAL_DOMAIN_SEPARATOR as INT, LEAF_DOMAIN_SEPARATOR as LEAF};
    type Node = PoseidonNode<Native>;
    let (a, b) = (digest(1), digest(2));
    let (pa, pb) = (Pos::export(&a), Pos::export(&b));
    let sponge = |bytes: &[u8]| Pos::export(&Pos::bytes(bytes));

    assert_eq!(
        stream_hash::<Node>(&[INT, &pa, &pb]),
        Pos::export(&Pos::internal(&a, &b))
    );
    assert_eq!(
        stream_hash::<Node>(&[LEAF, &pa, &pb]),
        Pos::export(&<Pos as JmtHasher>::leaf(&a, &b))
    );

    // A value equal to a separator, or to a whole node's bytes, stays a byte hash.
    assert_eq!(stream_hash::<Node>(&[INT]), sponge(INT));
    assert_eq!(stream_hash::<Node>(&[LEAF]), sponge(LEAF));
    let whole = [INT, &pa, &pb].concat();
    assert_eq!(stream_hash::<Node>(&[&whole]), sponge(&whole));
    assert_eq!(stream_hash::<Node>(&[]), sponge(&[]));
    assert_eq!(stream_hash::<Node>(&[&pa]), sponge(&pa));
    // A node shape that does not finish, or goes on, also stays a byte hash.
    assert_eq!(
        stream_hash::<Node>(&[INT, &pa]),
        sponge(&[INT, &pa].concat())
    );
    assert_eq!(
        stream_hash::<Node>(&[INT, &pa, &pb, b"x"]),
        sponge(&[INT, &pa, &pb, b"x"].concat())
    );
    assert_eq!(
        stream_hash::<Node>(&[INT, b"short", &pb]),
        sponge(&[INT, b"short", &pb].concat())
    );
}

/// The vector from SP1's own tests, so this is the permutation behind the precompile.
#[test]
fn poseidon_permutation_matches_the_known_answer() {
    use merkle_map::hash::{PermBackend, PermState};
    let mut state = PermState([0; 16]);
    // SAFETY: the state is 16 canonical elements.
    unsafe { Native::permute(&mut state) };
    assert_eq!(
        state.0,
        [
            145589356, 1876041682, 1734203622, 499355069, 673349476, 595701365, 270340205,
            131707822, 1236787881, 1085405948, 2065733208, 1999012278, 2062318124, 1616707536,
            324813015, 749520722,
        ]
    );
}

// ---------------------------------------------------------------------------------------------
// Indexed map
// ---------------------------------------------------------------------------------------------

#[allow(clippy::map_entry)]
fn imm_model<H: ImmHasher>() {
    const HEIGHT: usize = 10;
    let mut map = IndexedMerkleMap::<H>::new(HEIGHT);
    let mut model = BTreeMap::new();
    for step in 0..400u64 {
        let key = digest(1 + step % 150);
        let value = digest(10_000 + step);
        let (internal, length) = (map.internal_root(), map.length());
        if model.contains_key(&key) {
            let witness = map.update_and_generate_witness(key, value).unwrap();
            let old = model.insert(key, value).unwrap();
            let new = ProvableIndexedMerkleMap::<H>::update(
                &internal,
                &witness,
                &key,
                &old,
                &value,
                HEIGHT - 1,
            )
            .unwrap();
            assert_eq!(new, map.internal_root());
            assert_eq!(map.length(), length);
        } else {
            let witness = map.insert_and_generate_witness(key, value).unwrap();
            model.insert(key, value);
            let new = ProvableIndexedMerkleMap::<H>::insert(
                &internal,
                length,
                &witness,
                &key,
                &value,
                HEIGHT - 1,
            )
            .unwrap();
            assert_eq!(new, map.internal_root());
            assert_eq!(map.length(), length + 1);
        }
        assert_eq!(map.get_option(&key), Some(value));
    }

    // The linked list visits every key in order.
    let mut walked = Vec::new();
    let mut key = map.leaf(0).next_key;
    while key != Digest::ZERO {
        walked.push(key);
        key = map.leaf(map.index_of(&key).unwrap()).next_key;
    }
    assert_eq!(walked, model.keys().copied().collect::<Vec<_>>());

    // Loading the same leaves at once gives the same tree. Insert order sets leaf positions.
    let ordered: Vec<(Digest, Digest)> = (1..map.length())
        .map(|index| (map.leaf(index).key, map.leaf(index).value))
        .collect();
    let bulk = IndexedMerkleMap::<H>::bulk_load(HEIGHT, ordered.into_iter());
    assert_eq!(bulk.root(), map.root());
}

#[test]
fn imm_tracks_a_plain_map_with_sha256() {
    imm_model::<Sha>();
}

#[test]
fn imm_tracks_a_plain_map_with_poseidon2() {
    imm_model::<Pos>();
}

#[test]
fn imm_crate_and_raw_sha256_build_the_same_tree() {
    let mut raw = IndexedMerkleMap::<Sha>::new(8);
    let mut krate = IndexedMerkleMap::<Sha2Crate<Native>>::new(8);
    assert_eq!(raw.root(), krate.root());
    for step in 0..60 {
        raw.insert(digest(step + 1), digest(step + 100)).unwrap();
        krate.insert(digest(step + 1), digest(step + 100)).unwrap();
        assert_eq!(raw.root(), krate.root());
    }
}

fn imm_capacity<H: ImmHasher>() {
    let mut map = IndexedMerkleMap::<H>::new(3);
    for step in 1..4 {
        map.insert(digest(step), digest(step)).unwrap();
    }
    assert_eq!(
        map.insert(digest(9), digest(9)),
        Err(IndexedMerkleMapError::TreeFull)
    );
    assert_eq!(
        map.insert(digest(1), digest(9)),
        Err(IndexedMerkleMapError::KeyAlreadyExists)
    );
    assert_eq!(
        map.update(digest(9), digest(9)),
        Err(IndexedMerkleMapError::KeyDoesNotExist)
    );
}

#[test]
fn imm_reports_capacity_and_key_errors() {
    imm_capacity::<Sha>();
    imm_capacity::<Pos>();
}

/// What a forged insert must not get past.
fn imm_insert_rejections<H: ImmHasher>() {
    const HEIGHT: usize = 6;
    const DEPTH: usize = HEIGHT - 1;
    type Provable<H> = ProvableIndexedMerkleMap<H>;
    let mut map = IndexedMerkleMap::<H>::new(HEIGHT);
    for step in 1..=7 {
        map.insert(digest(step * 11), digest(step)).unwrap();
    }
    let (root, length) = (map.internal_root(), map.length());
    let (key, value) = (digest(1000), digest(2000));
    let good = map.clone().insert_and_generate_witness(key, value).unwrap();
    let accepted = |witness: &InsertWitness| {
        Provable::<H>::insert(&root, length, witness, &key, &value, DEPTH)
    };
    assert_eq!(accepted(&good), {
        let mut applied = map.clone();
        applied.insert(key, value).unwrap();
        Ok(applied.internal_root())
    });

    // Forged slot siblings, the way to claim any new root.
    let mut forged = good.clone();
    forged.slot_proof.siblings[DEPTH - 1] = digest(777);
    assert_eq!(accepted(&forged), Err(IndexedMerkleMapError::RootMismatch));
    for level in 0..DEPTH {
        let mut forged = good.clone();
        forged.slot_proof.siblings[level] = digest(778 + level as u64);
        assert!(
            accepted(&forged).is_err(),
            "forged slot sibling at level {level}"
        );
    }

    // A slot that is not empty: another position's real siblings.
    let mut occupied = good.clone();
    occupied.slot_proof = map.merkle_proof(3);
    assert!(accepted(&occupied).is_err());

    // Slot siblings read before the low leaf changed, which only differ when the paths share levels.
    let mut stale = good.clone();
    stale.slot_proof = map.merkle_proof(length);
    assert!(accepted(&stale).is_err());

    // Proofs of the wrong depth.
    let mut short = good.clone();
    short.slot_proof.siblings.pop();
    assert_eq!(accepted(&short), Err(IndexedMerkleMapError::InvalidWitness));
    let mut long = good.clone();
    long.non_membership_proof
        .merkle_proof
        .siblings
        .push(digest(5));
    assert_eq!(accepted(&long), Err(IndexedMerkleMapError::InvalidProof));

    // A low leaf that does not bracket the key, or is not in the tree.
    let wrong = map.get_membership_proof(&digest(11)).unwrap();
    let mut predecessor = good.clone();
    predecessor.non_membership_proof.low_leaf = wrong.leaf;
    predecessor.non_membership_proof.index = wrong.index;
    predecessor.non_membership_proof.merkle_proof = wrong.merkle_proof;
    let low_key = good.non_membership_proof.low_leaf.key;
    if wrong.leaf.key != low_key {
        assert!(accepted(&predecessor).is_err());
    }
    let mut tampered = good.clone();
    tampered.non_membership_proof.low_leaf.next_key = Digest::ZERO;
    assert!(accepted(&tampered).is_err());
    let mut tampered = good.clone();
    tampered.non_membership_proof.index ^= 1;
    assert!(accepted(&tampered).is_err());
    let mut tampered = good.clone();
    tampered.non_membership_proof.merkle_proof.siblings[0] = digest(9);
    assert!(accepted(&tampered).is_err());

    // A key that is present, the zero key, and the wrong old root or length.
    let present = digest(11);
    assert!(Provable::<H>::insert(&root, length, &good, &present, &value, DEPTH).is_err());
    assert!(Provable::<H>::insert(&root, length, &good, &Digest::ZERO, &value, DEPTH).is_err());
    assert!(Provable::<H>::insert(&digest(1), length, &good, &key, &value, DEPTH).is_err());
    assert!(Provable::<H>::insert(&root, length, &good, &key, &value, DEPTH - 1).is_err());

    // A full tree has no slot, even if the bits would wrap to one.
    assert_eq!(
        Provable::<H>::insert(&root, 1 << DEPTH, &good, &key, &value, DEPTH),
        Err(IndexedMerkleMapError::TreeFull)
    );
}

#[test]
fn imm_insert_rejects_forgeries_with_sha256() {
    imm_insert_rejections::<Sha>();
}

#[test]
fn imm_insert_rejects_forgeries_with_poseidon2() {
    imm_insert_rejections::<Pos>();
}

/// A low leaf at the slot just before the new one shares all but the last level with it.
fn imm_shared_paths<H: ImmHasher>() {
    const HEIGHT: usize = 5;
    let mut map = IndexedMerkleMap::<H>::new(HEIGHT);
    map.insert(digest(1), digest(2)).unwrap();
    let mut keys: Vec<Digest> = (10..14).map(digest).collect();
    keys.sort_unstable();
    let last = map.length() - 1;
    let key = if keys[3] > map.leaf(last).key {
        keys[3]
    } else {
        digest(99)
    };
    let (root, length) = (map.internal_root(), map.length());
    let witness = map.insert_and_generate_witness(key, digest(3)).unwrap();
    let new = ProvableIndexedMerkleMap::<H>::insert(
        &root,
        length,
        &witness,
        &key,
        &digest(3),
        HEIGHT - 1,
    )
    .unwrap();
    assert_eq!(new, map.internal_root());
}

#[test]
fn imm_insert_handles_shared_paths() {
    imm_shared_paths::<Sha>();
    imm_shared_paths::<Pos>();
}

// ---------------------------------------------------------------------------------------------
// Engines against each other, and the batch verifier's rejections
// ---------------------------------------------------------------------------------------------

fn imm_engines_agree<H: ImmHasher>(chained: Arm, batch: Arm) {
    for (workload, k, state) in [
        (Workload::Insert, 40, 300),
        (Workload::Update, 40, 300),
        (Workload::Mixed, 60, 300),
        (Workload::Append, 40, 300),
        (Workload::Insert, 8, 1),
        (Workload::Append, 20, 7),
    ] {
        let base = point(chained, workload, k, state, 12, 1);
        let imm = ImmState::<H>::build(&base);
        let (witness, sidecar) = chained_witness(&base, &imm).unwrap();
        let reference = run(chained, &witness);
        assert_eq!(reference.new_root, sidecar.expected_new_root.unwrap());

        for sub_batches in [1, 5, k] {
            let mut this = point(batch, workload, k, state, 12, sub_batches);
            this.sub_batches = sub_batches;
            let (witness, sidecar) = batch_witness(&this, &imm).unwrap();
            let output = run(batch, &witness);
            assert_eq!(
                output.new_root,
                sidecar.expected_new_root.unwrap(),
                "{workload:?} in {sub_batches} blocks"
            );
            assert_eq!(
                output.new_root, reference.new_root,
                "{workload:?} in {sub_batches}"
            );
            assert_eq!(output.batch_hash, reference.batch_hash);
            assert_eq!(output.new_len, reference.new_len);
        }
    }
}

#[test]
fn imm_chained_and_batch_reach_one_root_with_sha256() {
    imm_engines_agree::<Sha>(Arm::ImmChainedSha256, Arm::ImmBatchSha256);
}

#[test]
fn imm_chained_and_batch_reach_one_root_with_poseidon2() {
    imm_engines_agree::<Pos>(Arm::ImmChainedPoseidon2, Arm::ImmBatchPoseidon2);
}

fn imm_records_build_on_each_other<H: ImmHasher>(chained: Arm, batch: Arm) {
    // Sequential keys make the order exact: 21 and 23 are new, 22's low leaf is the new 21, and 23
    // is then updated twice within the batch.
    let base = point(batch, Workload::Append, 5, 20, 10, 1);
    let imm = ImmState::<H>::build(&base);
    let key = |sequence: u64| sequence.to_le_bytes().to_vec();
    let payload = |i: u64| workload::initial_payload(99, 1000 + i, 24);
    let mut delta = vec![0u8; 24];
    delta[0] = 5;
    let records = vec![
        Rec {
            op: OP_INSERT,
            hint: 0,
            key: key(21),
            arg: payload(0),
        },
        Rec {
            op: OP_INSERT,
            hint: 0,
            key: key(23),
            arg: payload(2),
        },
        Rec {
            op: OP_INSERT,
            hint: 0,
            key: key(22),
            arg: payload(1),
        },
        Rec {
            op: OP_UPDATE,
            hint: 0,
            key: key(23),
            arg: delta.clone(),
        },
        Rec {
            op: OP_UPDATE,
            hint: 0,
            key: key(23),
            arg: delta.clone(),
        },
        Rec {
            op: OP_UPDATE,
            hint: 0,
            key: key(5),
            arg: delta.clone(),
        },
    ];
    let (witness, sidecar) = batch_witness_of(&base, &imm, records.clone()).unwrap();
    let output = run(batch, &witness);
    assert_eq!(output.new_root, sidecar.expected_new_root.unwrap());
    let (witness, sidecar) = chained_witness_of(&base, &imm, records).unwrap();
    let reference = run(chained, &witness);
    assert_eq!(reference.new_root, sidecar.expected_new_root.unwrap());
    assert_eq!(reference.new_root, output.new_root);
}

#[test]
fn imm_records_in_one_batch_can_build_on_each_other() {
    imm_records_build_on_each_other::<Sha>(Arm::ImmChainedSha256, Arm::ImmBatchSha256);
    imm_records_build_on_each_other::<Pos>(Arm::ImmChainedPoseidon2, Arm::ImmBatchPoseidon2);
}

/// A view of a batch witness, to tamper with.
struct BatchLayout {
    engine: usize,
    touched: usize,
    siblings: usize,
    records: usize,
    stride: usize,
}

fn batch_layout(witness: &[u8]) -> BatchLayout {
    let words = aligned(witness);
    // SAFETY: the buffer starts with a header.
    let header: Header = unsafe { as_bytes(&words).as_ptr().cast::<Header>().read() };
    let engine = HEADER_BYTES + (header.records_bytes + header.olds_bytes) as usize;
    let block: BlockHeader = unsafe {
        as_bytes(&words)[engine..]
            .as_ptr()
            .cast::<BlockHeader>()
            .read()
    };
    BatchLayout {
        engine,
        touched: block.touched as usize,
        siblings: block.siblings as usize,
        records: header.records as usize,
        stride: 8 + 88 + 24,
    }
}

const TOUCHED_BYTES: usize = size_of::<TouchedLeaf>();
const BLOCK_BYTES: usize = size_of::<BlockHeader>();

fn imm_batch_rejections<H: ImmHasher>(arm: Arm) {
    let base = point(arm, Workload::Mixed, 40, 200, 12, 1);
    let imm = ImmState::<H>::build(&base);
    let (good, _) = batch_witness(&base, &imm).unwrap();
    assert!(!rejects(arm, &good));
    let layout = batch_layout(&good);
    assert!(layout.touched >= 2 && layout.siblings >= 2);
    let leaves = layout.engine + BLOCK_BYTES;
    let siblings = leaves + layout.touched * TOUCHED_BYTES;

    let tampered = |change: &dyn Fn(&mut Vec<u8>)| {
        let mut witness = good.clone();
        change(&mut witness);
        witness
    };
    let check = |name: &str, change: &dyn Fn(&mut Vec<u8>)| {
        assert!(rejects(arm, &tampered(change)), "accepted: {name}");
    };

    check("a changed first sibling", &|w| w[siblings] ^= 1);
    check("a changed last sibling", &|w| {
        w[siblings + (layout.siblings - 1) * 32] ^= 1
    });
    check("a changed touched leaf key", &|w| w[leaves + 8] ^= 1);
    check("a changed touched leaf value", &|w| w[leaves + 8 + 32] ^= 1);
    check("a changed touched leaf next key", &|w| {
        w[leaves + 8 + 64] ^= 1
    });
    check("a touched leaf out of order", &|w| {
        let (first, second) = (leaves, leaves + TOUCHED_BYTES);
        let a: [u8; 8] = w[first..first + 8].try_into().unwrap();
        let b: [u8; 8] = w[second..second + 8].try_into().unwrap();
        w[first..first + 8].copy_from_slice(&b);
        w[second..second + 8].copy_from_slice(&a);
    });
    check("a repeated touched index", &|w| {
        let first: [u8; 8] = w[leaves..leaves + 8].try_into().unwrap();
        w[leaves + TOUCHED_BYTES..leaves + TOUCHED_BYTES + 8].copy_from_slice(&first);
    });
    check("a touched index past the old leaves", &|w| {
        let last = leaves + (layout.touched - 1) * TOUCHED_BYTES;
        w[last..last + 8].copy_from_slice(&(1u64 << 40).to_le_bytes());
    });
    check("a missing sibling", &|w| {
        let count = u64::from_le_bytes(
            w[layout.engine + 16..layout.engine + 24]
                .try_into()
                .unwrap(),
        );
        w[layout.engine + 16..layout.engine + 24].copy_from_slice(&(count - 1).to_le_bytes());
        let end = w.len();
        w.truncate(end - 32);
        patch_engine_bytes(w, -32);
    });
    check("an extra sibling", &|w| {
        let count = u64::from_le_bytes(
            w[layout.engine + 16..layout.engine + 24]
                .try_into()
                .unwrap(),
        );
        w[layout.engine + 16..layout.engine + 24].copy_from_slice(&(count + 1).to_le_bytes());
        w.extend_from_slice(&[0u8; 32]);
        patch_engine_bytes(w, 32);
    });
    check("a trailing witness", &|w| {
        w.extend_from_slice(&[0u8; 8]);
        patch_engine_bytes(w, 8);
    });
    check("a truncated witness", &|w| {
        let end = w.len();
        w.truncate(end - 8);
    });
    check("a wrong old root", &|w| {
        w[header_offset_old_root()] ^= 1;
    });
    check("a wrong old length", &|w| {
        w[header_offset_old_len()] ^= 2;
    });
    check("a wrong height", &|w| {
        w[header_offset_height()] ^= 1;
    });
    check("a hint past the end", &|w| {
        let meta = HEADER_BYTES;
        w[meta + 1..meta + 8].copy_from_slice(&[0xff, 0xff, 0, 0, 0, 0, 0]);
    });

    // Records are bound: changing one changes the commitment or fails.
    let original = run(arm, &good);
    for record in 0..layout.records.min(5) {
        let arg = HEADER_BYTES + record * layout.stride + 8 + 88;
        let changed = tampered(&|w: &mut Vec<u8>| w[arg] ^= 1);
        if !rejects(arm, &changed) {
            assert_ne!(run(arm, &changed).batch_hash, original.batch_hash);
        }
    }
}

/// Keeps a tampered witness's own size fields consistent, so the engine itself must catch it.
fn patch_engine_bytes(witness: &mut [u8], change: i64) {
    let offset = core::mem::offset_of!(Header, engine_bytes);
    let current = u64::from_le_bytes(witness[offset..offset + 8].try_into().unwrap());
    let updated = (current as i64 + change) as u64;
    witness[offset..offset + 8].copy_from_slice(&updated.to_le_bytes());
}

fn header_offset_old_root() -> usize {
    core::mem::offset_of!(Header, old_root)
}

fn header_offset_old_len() -> usize {
    core::mem::offset_of!(Header, old_len)
}

fn header_offset_height() -> usize {
    core::mem::offset_of!(Header, height)
}

/// The chained engine, on whole witnesses. The length it trusts comes from the bound root.
fn imm_chained_rejections<H: ImmHasher>(arm: Arm) {
    let base = point(arm, Workload::Mixed, 30, 200, 12, 1);
    let imm = ImmState::<H>::build(&base);
    let (good, _) = chained_witness(&base, &imm).unwrap();
    assert!(!rejects(arm, &good));
    let words = aligned(&good);
    // SAFETY: the buffer starts with a header.
    let header: Header = unsafe { as_bytes(&words).as_ptr().cast::<Header>().read() };
    let olds = HEADER_BYTES + header.records_bytes as usize;
    let engine = olds + header.olds_bytes as usize;

    let check = |name: &str, change: &dyn Fn(&mut Vec<u8>)| {
        let mut witness = good.clone();
        change(&mut witness);
        assert!(rejects(arm, &witness), "accepted: {name}");
    };
    check("a wrong old root", &|w| {
        w[core::mem::offset_of!(Header, old_root)] ^= 1
    });
    check("a wrong internal root", &|w| {
        w[core::mem::offset_of!(Header, old_internal)] ^= 1
    });
    check("a wrong old length", &|w| {
        w[core::mem::offset_of!(Header, old_len)] ^= 1
    });
    check("a wrong height", &|w| {
        w[core::mem::offset_of!(Header, height)] ^= 1
    });
    check("a wrong old payload", &|w| w[olds] ^= 1);
    check("a changed first witness byte", &|w| w[engine + 5] ^= 1);
    check("a changed last sibling", &|w| {
        let end = w.len();
        w[end - 9] ^= 1;
    });
    check("a truncated witness", &|w| {
        let end = w.len();
        w.truncate(end - 8);
    });
    check("a trailing witness", &|w| {
        w.extend_from_slice(&[1u8; 8]);
        patch_engine_bytes(w, 8);
    });
    check("a witness for another operation", &|w| {
        let meta = HEADER_BYTES;
        w[meta] = if w[meta] == OP_INSERT {
            OP_UPDATE
        } else {
            OP_INSERT
        };
    });
}

#[test]
fn imm_chained_rejects_tampering_with_sha256() {
    imm_chained_rejections::<Sha>(Arm::ImmChainedSha256);
}

#[test]
fn imm_chained_rejects_tampering_with_poseidon2() {
    imm_chained_rejections::<Pos>(Arm::ImmChainedPoseidon2);
}

#[test]
fn imm_batch_rejects_tampering_with_sha256() {
    imm_batch_rejections::<Sha>(Arm::ImmBatchSha256);
}

#[test]
fn imm_batch_rejects_tampering_with_poseidon2() {
    imm_batch_rejections::<Pos>(Arm::ImmBatchPoseidon2);
}

#[test]
fn imm_batch_rejects_records_that_break_the_list() {
    let arm = Arm::ImmBatchSha256;
    let base = point(arm, Workload::Insert, 6, 50, 8, 1);
    let imm = ImmState::<Sha>::build(&base);

    // Two inserts of one key: the host refuses, so build the witness for one and repeat the record.
    let (good, _) = batch_witness(&base, &imm).unwrap();
    let layout = batch_layout(&good);
    let first = HEADER_BYTES;
    let mut repeated = good.clone();
    let record: Vec<u8> = repeated[first..first + layout.stride].to_vec();
    repeated[first + layout.stride..first + 2 * layout.stride].copy_from_slice(&record);
    assert!(rejects(arm, &repeated), "accepted the same key twice");

    // A hint to an entry that does not exist yet.
    let mut forward = good.clone();
    let future = (layout.touched + 3) as u64;
    forward[first..first + 8].copy_from_slice(&((OP_INSERT as u64) | (future << 8)).to_le_bytes());
    assert!(rejects(arm, &forward), "accepted a forward hint");

    // The host refuses a batch past capacity, and a guest told the tree is lower fails its root.
    let tight = point(arm, Workload::Insert, 3, 5, 4, 1);
    let imm = ImmState::<Sha>::build(&tight);
    assert!(
        batch_witness(&tight, &imm).is_err(),
        "1 + 5 + 3 leaves do not fit in 8"
    );
    let fits = point(arm, Workload::Insert, 2, 5, 4, 1);
    assert!(!rejects(arm, &batch_witness(&fits, &imm).unwrap().0));
}

// ---------------------------------------------------------------------------------------------
// Jellyfish
// ---------------------------------------------------------------------------------------------

/// The tree's definition, independent of `jmt`: a leaf at the shortest unique prefix, the placeholder
/// for nothing, an internal node for anything else.
fn oracle<H: JmtHasher>(leaves: &[(Digest, Digest)], depth: u32) -> Digest {
    match leaves {
        [] => H::PLACEHOLDER,
        [(key, value)] => H::leaf(key, value),
        _ => {
            let bit = |key: &Digest| {
                let path = H::path_key(key);
                (path.0[depth as usize / 32] >> (31 - depth % 32)) & 1 == 1
            };
            let split = leaves.partition_point(|(key, _)| !bit(key));
            H::internal(
                &oracle::<H>(&leaves[..split], depth + 1),
                &oracle::<H>(&leaves[split..], depth + 1),
            )
        }
    }
}

fn jmt_root<S: SimpleHasher>(pairs: &[(KeyHash, Vec<u8>)]) -> [u8; 32] {
    if pairs.is_empty() {
        return JellyfishMerkleTree::<jmt_state::Store, S>::EMPTY_ROOT.0;
    }
    let store = jmt_state::Store::default();
    let (roots, _) = JellyfishMerkleTree::<_, S>::new(&store)
        .batch_put_value_sets(vec![pairs.to_vec()], None, 0)
        .unwrap();
    roots[0].0
}

/// Keys sharing a prefix of the given number of bits, built in path order for this hash.
fn keys_with_prefix<H: Hasher>(prefix_bits: usize, count: u64) -> Vec<[u8; 32]> {
    (0..count)
        .map(|index| {
            let mut key = [0u8; 32];
            // Fill the rest from the seed, then force the shared prefix to zeros.
            let filler = digest(index + 1);
            key.copy_from_slice(&H::export(&filler));
            for bit in 0..prefix_bits.min(256) {
                key[bit / 8] &= !(0x80 >> (bit % 8));
            }
            // Make the keys differ just past the prefix.
            if prefix_bits < 256 {
                let bit = prefix_bits;
                if index % 2 == 1 {
                    key[bit / 8] |= 0x80 >> (bit % 8);
                } else {
                    key[bit / 8] &= !(0x80 >> (bit % 8));
                }
            }
            // Stay canonical for Poseidon2: the top byte of each little-endian limb is below 0x7f.
            for limb in 0..8 {
                key[4 * limb + 3] &= 0x7f;
                if key[4 * limb + 3] == 0x7f {
                    key[4 * limb + 3] = 0x7e;
                }
            }
            key
        })
        .collect()
}

fn oracle_matches_jmt<H: Hasher + JmtHasher, S: SimpleHasher>() {
    for prefix in [0usize, 5, 8, 9, 63, 64, 252, 255] {
        for count in [0u64, 1, 2, 3, 7] {
            let mut pairs: BTreeMap<[u8; 32], Vec<u8>> = BTreeMap::new();
            for (index, key) in keys_with_prefix::<H>(prefix, count).into_iter().enumerate() {
                pairs.insert(key, vec![index as u8 + 1; 24]);
            }
            let as_jmt: Vec<(KeyHash, Vec<u8>)> = pairs
                .iter()
                .map(|(key, value)| (KeyHash(*key), value.clone()))
                .collect();
            let as_oracle: Vec<(Digest, Digest)> = pairs
                .iter()
                .map(|(key, value)| (H::import(key), H::import(&S::hash(value))))
                .collect();
            let expected = H::export(&oracle::<H>(&as_oracle, 0));
            assert_eq!(
                jmt_root::<S>(&as_jmt),
                expected,
                "prefix {prefix}, {count} keys"
            );
        }
    }
}

#[test]
fn the_tree_definition_matches_jmt_with_sha256() {
    oracle_matches_jmt::<Sha, sha2::Sha256>();
}

#[test]
fn the_tree_definition_matches_jmt_with_poseidon2() {
    oracle_matches_jmt::<Pos, PoseidonNode<Native>>();
}

fn jmt_engines_agree<H: Hasher + JmtHasher, S: SimpleHasher>(stock: Arm, batch: Arm) {
    for (workload, k, state) in [
        (Workload::Insert, 40, 500),
        (Workload::Update, 40, 500),
        (Workload::Mixed, 60, 500),
        (Workload::Append, 30, 500),
        (Workload::Insert, 40, 1),
        (Workload::Insert, 20, 3),
        (Workload::Insert, 30, 0),
    ] {
        let base = point(stock, workload, k, state, 0, 1);
        let tree = State::<S>::build(7, state, base.key_size as usize, 24).unwrap();

        let (witness, sidecar) = jmt_state::stock_witness::<H, S>(&base, &tree).unwrap();
        let reference = run(stock, &witness);
        assert_eq!(
            reference.new_root,
            sidecar.expected_new_root.unwrap(),
            "{workload:?} {state}"
        );

        let this = point(batch, workload, k, state, 0, 1);
        let (witness, sidecar) = jmt_state::batch_witness::<H, S>(&this, &tree).unwrap();
        let output = run(batch, &witness);
        assert_eq!(
            output.new_root,
            sidecar.expected_new_root.unwrap(),
            "{workload:?} {state}"
        );
        assert_eq!(output.new_root, reference.new_root);
        assert_eq!(output.batch_hash, reference.batch_hash);
    }
}

#[test]
fn jmt_stock_and_batch_reach_one_root_with_sha256() {
    jmt_engines_agree::<Sha, sha2::Sha256>(Arm::JmtStockSha256, Arm::JmtBatchSha256);
}

#[test]
fn jmt_stock_and_batch_reach_one_root_with_poseidon2() {
    jmt_engines_agree::<Pos, PoseidonNode<Native>>(
        Arm::JmtStockPoseidon2Node,
        Arm::JmtBatchPoseidon2,
    );
}

/// Records with chosen keys, for building trees with exact shapes.
fn crafted_stock_witness(existing: &[[u8; 32]], writes: &[([u8; 32], bool)]) -> (Vec<u8>, Vec<u8>) {
    type S = Sha256Stream<Native>;
    let store = jmt_state::Store::default();
    let pairs: Vec<(KeyHash, Vec<u8>)> = existing
        .iter()
        .map(|key| (KeyHash(*key), vec![1u8; 24]))
        .collect();
    let (roots, batch) = JellyfishMerkleTree::<_, S>::new(&store)
        .batch_put_value_sets(vec![pairs], None, 0)
        .unwrap();
    let mut store = store;
    store.apply(&batch);
    let tree = JellyfishMerkleTree::<_, S>::new(&store);
    let value_sets: Vec<(KeyHash, Option<Vec<u8>>)> = writes
        .iter()
        .map(|(key, _)| (KeyHash(*key), Some(vec![2u8; 24])))
        .collect();
    let (new_root, proof, _) = tree.put_value_set_with_proof(value_sets, 1).unwrap();
    (
        borsh::to_vec(&proof).unwrap(),
        [roots[0].0, new_root.0].concat(),
    )
}

/// The guard's byte offsets must read what the crate's decoder reads.
#[test]
fn jmt_guard_reads_the_real_proof_layout() {
    use jmt::{ValueHash, proof::SparseMerkleProof, storage::LeafNode};
    type S = Sha256Stream<Native>;
    let keys: Vec<[u8; 32]> = (0..200u64)
        .map(|i| Sha::export(&Sha::bytes(&i.to_le_bytes())))
        .collect();
    let (bytes, _) = crafted_stock_witness(&keys, &[(keys[5], true)]);
    let mut frames = Vec::new();
    jmt_state::frame_update_proof(&bytes, &mut frames);
    let frame = &frames[4..];
    let proof: SparseMerkleProof<S> = borsh::from_slice(frame).unwrap();
    assert_eq!(frame[0], 1, "this proof has a leaf");
    let key: [u8; 32] = frame[1..33].try_into().unwrap();
    let value: [u8; 32] = frame[33..65].try_into().unwrap();
    assert_eq!(key, keys[5]);
    let expected = LeafNode::new(KeyHash(key), ValueHash(value));
    assert_eq!(proof.leaf(), Some(expected.into()));
    let siblings = u32::from_le_bytes(frame[65..69].try_into().unwrap());
    assert!(siblings > 0 && siblings <= 256);
    merkle_map::jellyfish::guard::check_update(frame, &key, &value);
}

/// A leaf, and the proof to it in a tree of two leaves that differ in the first bit.
fn two_leaf_proof() -> (Vec<u8>, [u8; 32], [u8; 32]) {
    let (mut low, mut high) = ([0u8; 32], [0u8; 32]);
    low[31] = 1;
    high[0] = 0x80;
    let (bytes, _) = crafted_stock_witness(&[low, high], &[(low, true)]);
    let mut frames = Vec::new();
    jmt_state::frame_update_proof(&bytes, &mut frames);
    (frames[4..].to_vec(), low, high)
}

#[test]
fn jmt_guard_rejects_a_key_that_does_not_belong_under_the_leaf() {
    use merkle_map::jellyfish::guard::check_insert;
    let (frame, low, high) = two_leaf_proof();

    // One bit short of the sibling count.
    let mut outside = high;
    outside[0] = 0x81;
    assert!(panics(|| check_insert(&frame, &outside)));

    // The same key as the leaf.
    assert!(panics(|| check_insert(&frame, &low)));

    // A key under the leaf passes: it shares the one sibling's worth of prefix.
    let mut under = low;
    under[31] = 3;
    check_insert(&frame, &under);

    // A deeper proof, with the key far outside it.
    let keys: Vec<[u8; 32]> = (0..2000u64)
        .map(|i| Sha::export(&Sha::bytes(&i.to_le_bytes())))
        .collect();
    let (bytes, _) = crafted_stock_witness(&keys, &[(keys[9], true)]);
    let mut frames = Vec::new();
    jmt_state::frame_update_proof(&bytes, &mut frames);
    let mut far = keys[9];
    far[0] ^= 0x80;
    assert!(panics(|| check_insert(&frames[4..], &far)));
    far = keys[9];
    let siblings = u32::from_le_bytes(frames[4 + 65..4 + 69].try_into().unwrap()) as usize;
    let bit = siblings - 1;
    far[bit / 8] ^= 0x80 >> (bit % 8);
    assert!(panics(|| check_insert(&frames[4..], &far)));
}

#[test]
fn jmt_guard_rejects_bad_updates_and_headers() {
    use merkle_map::jellyfish::guard::{check_insert, check_update};
    let (frame, low, high) = two_leaf_proof();
    let value: [u8; 32] = frame[33..65].try_into().unwrap();

    check_update(&frame, &low, &value);
    assert!(
        panics(|| check_update(&frame, &high, &value)),
        "foreign leaf"
    );
    assert!(
        panics(|| check_update(&frame, &low, &[9u8; 32])),
        "wrong old value"
    );

    let mut none = vec![0u8];
    none.extend_from_slice(&0u32.to_le_bytes());
    assert!(panics(|| check_update(&none, &low, &value)), "absent key");
    check_insert(&none, &low);

    let mut many = vec![0u8];
    many.extend_from_slice(&257u32.to_le_bytes());
    assert!(panics(|| check_insert(&many, &low)), "too many siblings");
    assert!(panics(|| check_insert(&[7u8; 8], &low)), "bad leaf tag");
}

fn jmt_stock_rejections<H: Hasher + JmtHasher, S: SimpleHasher>(arm: Arm) {
    let base = point(arm, Workload::Update, 12, 200, 0, 1);
    let tree = State::<S>::build(7, 200, 84, 24).unwrap();
    let (good, _) = jmt_state::stock_witness::<H, S>(&base, &tree).unwrap();
    assert!(!rejects(arm, &good));

    let insert_point = point(arm, Workload::Insert, 12, 200, 0, 1);
    let (insert_good, _) = jmt_state::stock_witness::<H, S>(&insert_point, &tree).unwrap();
    assert!(!rejects(arm, &insert_good));

    // Each engine region starts with a frame length, then the proof.
    let words = aligned(&good);
    let header: Header = unsafe { as_bytes(&words).as_ptr().cast::<Header>().read() };
    let engine = HEADER_BYTES + (header.records_bytes + header.olds_bytes) as usize;
    let first_sibling = engine + 4 + 65 + 4 + 1;

    let check = |name: &str, change: &dyn Fn(&mut Vec<u8>)| {
        let mut witness = good.clone();
        change(&mut witness);
        assert!(rejects(arm, &witness), "accepted: {name}");
    };
    check("a changed sibling", &|w| w[first_sibling] ^= 1);
    check("a changed leaf value", &|w| w[engine + 4 + 33] ^= 1);
    check("a changed leaf key", &|w| w[engine + 4 + 1] ^= 1);
    check("a wrong old root", &|w| {
        w[core::mem::offset_of!(Header, old_root)] ^= 1
    });
    check("a wrong new root", &|w| {
        w[core::mem::offset_of!(Header, new_root)] ^= 1
    });
    check("a wrong old payload", &|w| {
        w[HEADER_BYTES + header.records_bytes as usize] ^= 1
    });
    check("a truncated witness", &|w| {
        let end = w.len();
        w.truncate(end - 8);
    });
    check("a trailing witness", &|w| {
        w.extend_from_slice(&[1u8; 8]);
        patch_engine_bytes(w, 8);
    });

    // Proofs for another batch's writes.
    let other = point(arm, Workload::Update, 12, 200, 0, 1);
    let mut other_with_seed = other.clone();
    other_with_seed.seed = 8;
    let (other_witness, _) = jmt_state::stock_witness::<H, S>(
        &other_with_seed,
        &State::<S>::build(8, 200, 84, 24).unwrap(),
    )
    .unwrap();
    let mut mixed = good.clone();
    let tail = HEADER_BYTES + header.records_bytes as usize;
    mixed.truncate(tail);
    mixed.extend_from_slice(&other_witness[tail..]);
    let other_words = aligned(&other_witness);
    let other_header: Header = unsafe { as_bytes(&other_words).as_ptr().cast::<Header>().read() };
    let offset = core::mem::offset_of!(Header, engine_bytes);
    mixed[offset..offset + 8].copy_from_slice(&other_header.engine_bytes.to_le_bytes());
    assert!(
        rejects(arm, &mixed),
        "accepted proofs for a different write set"
    );
}

#[test]
fn jmt_stock_rejects_tampering_with_sha256() {
    jmt_stock_rejections::<Sha, Sha256Stream<Native>>(Arm::JmtStockSha256);
}

#[test]
fn jmt_stock_rejects_tampering_with_poseidon2() {
    jmt_stock_rejections::<Pos, PoseidonNode<Native>>(Arm::JmtStockPoseidon2Node);
}

fn jmt_batch_rejections<H: Hasher + JmtHasher, S: SimpleHasher>(arm: Arm) {
    use merkle_map::jellyfish::batch::{TAG_BRANCH, TAG_HASH, TAG_LEAF, TAG_NULL};
    let tree = State::<S>::build(7, 300, 84, 24).unwrap();
    for workload in [Workload::Update, Workload::Insert, Workload::Mixed] {
        let base = point(arm, workload, 20, 300, 0, 1);
        let (good, _) = jmt_state::batch_witness::<H, S>(&base, &tree).unwrap();
        assert!(!rejects(arm, &good));

        let words = aligned(&good);
        let header: Header = unsafe { as_bytes(&words).as_ptr().cast::<Header>().read() };
        let engine = HEADER_BYTES + (header.records_bytes + header.olds_bytes) as usize;
        let tag_count = u64::from_le_bytes(good[engine..engine + 8].try_into().unwrap()) as usize;
        let digest_count =
            u64::from_le_bytes(good[engine + 8..engine + 16].try_into().unwrap()) as usize;
        let tags = engine + 16;
        let digests = tags + tag_count.next_multiple_of(8);
        assert!(digest_count > 2);

        let check = |name: &str, change: &dyn Fn(&mut Vec<u8>)| {
            let mut witness = good.clone();
            change(&mut witness);
            assert!(rejects(arm, &witness), "{workload:?} accepted: {name}");
        };
        check("a changed digest", &|w| w[digests] ^= 1);
        check("a changed last digest", &|w| {
            w[digests + (digest_count - 1) * 32] ^= 1
        });
        check("a wrong old root", &|w| {
            w[core::mem::offset_of!(Header, old_root)] ^= 1
        });
        check("a bad tag", &|w| w[tags] = 9);
        check("a first tag that is not a branch", &|w| w[tags] = TAG_HASH);
        check("a truncated witness", &|w| {
            let end = w.len();
            w.truncate(end - 32);
        });

        // Every leaf and hash tag swapped for another shape must fail: a record cannot be routed
        // into an opaque hash, and an empty slot cannot stand in for a real leaf.
        let tag_slice = good[tags..tags + tag_count].to_vec();
        for (index, tag) in tag_slice.iter().enumerate().take(200) {
            let replacement = match *tag {
                TAG_LEAF => TAG_HASH,
                TAG_HASH => TAG_NULL,
                TAG_BRANCH => TAG_NULL,
                _ => TAG_HASH,
            };
            let mut witness = good.clone();
            witness[tags + index] = replacement;
            assert!(
                rejects(arm, &witness),
                "{workload:?} accepted tag {tag} replaced by {replacement} at {index}"
            );
        }

        // A trailing tag or digest.
        let mut extra = good.clone();
        extra[engine + 8..engine + 16].copy_from_slice(&(digest_count as u64 + 1).to_le_bytes());
        extra.extend_from_slice(&[0u8; 32]);
        patch_engine_bytes(&mut extra, 32);
        assert!(
            rejects(arm, &extra),
            "{workload:?} accepted a trailing digest"
        );
    }
}

#[test]
fn jmt_batch_rejects_tampering_with_sha256() {
    jmt_batch_rejections::<Sha, sha2::Sha256>(Arm::JmtBatchSha256);
}

#[test]
fn jmt_batch_rejects_tampering_with_poseidon2() {
    jmt_batch_rejections::<Pos, PoseidonNode<Native>>(Arm::JmtBatchPoseidon2);
}

/// A stop point comes from the witness, so stopping must never commit a root nothing verified, and
/// the public values must show that the run stopped.
fn stopped_runs_prove_no_transition(arm: Arm, witness: &[u8]) {
    let full = run(arm, witness);
    assert_ne!(full.new_root, [0u8; 32], "{}", arm.name());
    assert_eq!(full.stop_after, 0);
    for stop in [1u32, 2] {
        let mut stopped = witness.to_vec();
        let offset = core::mem::offset_of!(Header, stop_after);
        stopped[offset..offset + 4].copy_from_slice(&stop.to_le_bytes());
        let output = run(arm, &stopped);
        assert_eq!(
            output.new_root,
            [0u8; 32],
            "{} stopped at {stop}",
            arm.name()
        );
        assert_eq!(output.new_len, 0, "{} stopped at {stop}", arm.name());
        assert_eq!(output.stop_after, stop, "{} stopped at {stop}", arm.name());
        assert_ne!(output.as_bytes(), full.as_bytes());
    }
}

#[test]
fn stopped_runs_commit_no_new_root() {
    for workload in [Workload::Insert, Workload::Update, Workload::Mixed] {
        for arm in Arm::ALL {
            let this = point(arm, workload, 5, 100, 10, 1);
            let (witness, _) = merkle_bench::prepare::witness_for(&this).unwrap();
            stopped_runs_prove_no_transition(arm, &witness);
        }
    }
}

/// Settings the witness chooses change the public values, so they cannot be swapped unseen.
#[test]
fn witness_settings_are_committed() {
    let this = point(Arm::ImmBatchPoseidon2, Workload::Insert, 5, 100, 10, 1);
    let (witness, _) = merkle_bench::prepare::witness_for(&this).unwrap();
    let output = run(Arm::ImmBatchPoseidon2, &witness);
    assert_eq!(
        (output.key_size, output.value_size, output.flags),
        (84, 24, 0)
    );
    let mut external = witness.clone();
    let offset = core::mem::offset_of!(Header, flags);
    external[offset] |= merkle_map::record::FLAG_EXTERNAL_BATCH_HASH as u8;
    let changed = run(Arm::ImmBatchPoseidon2, &external);
    assert_ne!(changed.flags, output.flags);
    assert_ne!(changed.batch_hash, output.batch_hash);
}

/// Only the stop points that exist count as measurements. Any other value must not pass as a run
/// that claims to be something else.
#[test]
fn unknown_stop_points_are_rejected() {
    for arm in Arm::ALL {
        let this = point(arm, Workload::Insert, 3, 40, 10, 1);
        let (mut witness, _) = merkle_bench::prepare::witness_for(&this).unwrap();
        let offset = core::mem::offset_of!(Header, stop_after);
        witness[offset..offset + 4].copy_from_slice(&3u32.to_le_bytes());
        assert_rejected(arm, &witness, "unknown stop point");
    }
}

/// Sizes chosen so a multiplication wraps must not pass for an empty batch or region.
#[test]
fn witness_sizes_that_wrap_are_rejected() {
    // The record size is 2^33, so 2^31 records would wrap to an empty region.
    for arm in Arm::ALL {
        let header = Header {
            records: 1 << 31,
            key_size: u32::MAX,
            value_size: u32::MAX - 7,
            sub_batches: 0,
            ..Default::default()
        };
        let (witness, _) = merkle_bench::prepare::assemble(header, &[], &[], &[]);
        assert_rejected(arm, &witness, "wrong record count");
    }

    // A touched leaf is 104 bytes and a digest is 32, so these counts would wrap to nothing.
    let arm = Arm::ImmBatchSha256;
    let (good, _) =
        merkle_bench::prepare::witness_for(&point(arm, Workload::Insert, 3, 40, 10, 1)).unwrap();
    let engine = batch_layout(&good).engine;
    let mut witness = good.clone();
    witness[engine + 8..engine + 16].copy_from_slice(&(1u64 << 61).to_le_bytes());
    assert_rejected(arm, &witness, "too many touched leaves");
    let mut witness = good;
    witness[engine + 16..engine + 24].copy_from_slice(&(1u64 << 59).to_le_bytes());
    assert_rejected(arm, &witness, "too many siblings");

    let arm = Arm::JmtBatchSha256;
    let (mut witness, _) =
        merkle_bench::prepare::witness_for(&point(arm, Workload::Insert, 3, 40, 0, 1)).unwrap();
    let (header, ..) = regions(&witness);
    let engine = HEADER_BYTES + (header.records_bytes + header.olds_bytes) as usize;
    let count = u64::from_le_bytes(witness[engine + 8..engine + 16].try_into().unwrap());
    witness[engine + 8..engine + 16].copy_from_slice(&(count + (1 << 59)).to_le_bytes());
    assert_rejected(arm, &witness, "wrong witness size");
}

#[test]
fn a_partial_old_payload_is_rejected() {
    for arm in [Arm::ImmBatchSha256, Arm::JmtBatchSha256] {
        let height = if arm.is_jmt() { 0 } else { 10 };
        let this = point(arm, Workload::Update, 3, 40, height, 1);
        let (witness, _) = merkle_bench::prepare::witness_for(&this).unwrap();
        let (header, records, mut olds, engine) = regions(&witness);
        olds.extend_from_slice(&[0u8; 8]);
        let (witness, _) = merkle_bench::prepare::assemble(header, &records, &olds, &engine);
        assert_rejected(arm, &witness, "partial old payload");
    }
}

/// The batch commitment under the external flag, recomputed from the records with only the `sha2`
/// crate, as a verifier outside the proof would. It must not need the map's own hash.
#[test]
fn the_external_batch_hash_needs_only_sha256_and_the_records() {
    use merkle_map::record::{ABI_VERSION, Witness};

    let sha256 = |parts: &[&[u8]]| -> [u8; 32] {
        let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
        for part in parts {
            sha2::Digest::update(&mut hasher, part);
        }
        sha2::Digest::finalize(hasher).into()
    };
    for arm in [
        Arm::ImmChainedPoseidon2,
        Arm::ImmBatchPoseidon2,
        Arm::JmtStockPoseidon2Node,
        Arm::JmtStockPoseidon2Sponge,
        Arm::JmtBatchPoseidon2,
    ] {
        let this = Point {
            external_batch_hash: true,
            ..point(arm, Workload::Mixed, 6, 50, 10, 1)
        };
        let (witness, _) = merkle_bench::prepare::witness_for(&this).unwrap();
        let output = run(arm, &witness);

        let words = aligned(&witness);
        let witness = Witness::parse(as_bytes(&words));
        let seed: [u32; 8] = [
            ABI_VERSION,
            output.state_version & 0xff_ffff,
            witness.header.records,
            0,
            0,
            0,
            0,
            0,
        ];
        let mut chain = [0u8; 32];
        for (bytes, word) in chain.as_chunks_mut::<4>().0.iter_mut().zip(seed) {
            *bytes = word.to_be_bytes();
        }
        for record in witness.records() {
            chain = sha256(&[
                &(record.op as u32).to_be_bytes(),
                &chain,
                &sha256(&[record.key]),
                &sha256(&[record.arg]),
            ]);
        }
        assert_eq!(output.batch_hash, chain, "{}", arm.name());
    }
}

/// The last step of a sponge is the same permutation call as a node. A leaf that took the same half
/// of that state would also be a node over its value hash, in a tree whose depth is not fixed.
#[test]
fn a_poseidon2_jellyfish_leaf_is_not_a_node() {
    use merkle_map::hash::{PermBackend, PermState, tag};

    let (key, value) = (digest(1), digest(2));
    let permute = |left: &[u32; 8], right: &[u32; 8]| -> PermState {
        let mut state = PermState([0; 16]);
        state.0[..8].copy_from_slice(left);
        state.0[8..].copy_from_slice(right);
        // SAFETY: the state holds 16 canonical elements.
        unsafe { Native::permute(&mut state) };
        state
    };
    let mut tagged = [0u32; 8];
    tagged[7] = tag::JMT_LEAF;
    let after_key = permute(&key.0, &tagged);
    let capacity = Digest(core::array::from_fn(|i| after_key.0[8 + i]));
    let last = permute(&value.0, &capacity.0);

    let leaf = <Pos as JmtHasher>::leaf(&key, &value);
    assert_eq!(leaf.0, last.0[8..]);
    let node = <Pos as JmtHasher>::internal(&value, &capacity);
    assert_eq!(node.0, last.0[..8]);
    assert_ne!(leaf, node);
}

/// The tree a Jellyfish batch witness spells out, in full control of its shape.
enum Shape {
    Null,
    Hash(Digest),
    Leaf(Digest, Digest),
    Branch(Box<Shape>, Box<Shape>),
}

impl Shape {
    fn branch(left: Shape, right: Shape) -> Self {
        Self::Branch(Box::new(left), Box::new(right))
    }

    /// A branch with `node` on the side `bit` says and `other` on the other side.
    fn side(bit: bool, node: Shape, other: Shape) -> Self {
        if bit {
            Self::branch(other, node)
        } else {
            Self::branch(node, other)
        }
    }

    fn root<H: JmtHasher>(&self) -> Digest {
        match self {
            Self::Null => H::PLACEHOLDER,
            Self::Hash(hash) => *hash,
            Self::Leaf(key, value) => H::leaf(key, value),
            Self::Branch(left, right) => H::internal(&left.root::<H>(), &right.root::<H>()),
        }
    }

    fn write(&self, tags: &mut Vec<u8>, digests: &mut Vec<u8>) {
        use merkle_map::jellyfish::batch::{TAG_BRANCH, TAG_HASH, TAG_LEAF, TAG_NULL};
        let mut push = |digest: &Digest| {
            for word in digest.0 {
                digests.extend_from_slice(&word.to_le_bytes());
            }
        };
        match self {
            Self::Null => tags.push(TAG_NULL),
            Self::Hash(hash) => {
                tags.push(TAG_HASH);
                push(hash);
            }
            Self::Leaf(key, value) => {
                tags.push(TAG_LEAF);
                push(key);
                push(value);
            }
            Self::Branch(left, right) => {
                tags.push(TAG_BRANCH);
                left.write(tags, digests);
                right.write(tags, digests);
            }
        }
    }

    fn witness(&self, old_root: Digest, records: &[Rec], olds: &[u8]) -> Vec<u8> {
        let (mut tags, mut digests) = (Vec::new(), Vec::new());
        self.write(&mut tags, &mut digests);
        let mut engine = Vec::new();
        engine.extend_from_slice(&(tags.len() as u64).to_le_bytes());
        engine.extend_from_slice(&(digests.len() as u64 / 32).to_le_bytes());
        engine.extend_from_slice(&tags);
        engine.resize(engine.len().next_multiple_of(8), 0);
        engine.extend_from_slice(&digests);
        let header = Header {
            records: records.len() as u32,
            key_size: 84,
            value_size: 24,
            sub_batches: 1,
            old_root,
            ..Default::default()
        };
        let records = workload::encode_records(records, 84, 24);
        merkle_bench::prepare::assemble(header, &records, olds, &engine).0
    }
}

fn shape_path_bit(path: &Digest, depth: u32) -> bool {
    (path.0[depth as usize / 32] >> (31 - depth % 32)) & 1 == 1
}

fn jmt_batch_rejects_shapes<H: Hasher + JmtHasher>(arm: Arm) {
    let preimage = |index: u64| workload::key_preimage(5, index, 84);
    let path_of = |key: &[u8]| H::path_key(&H::bytes(key));
    let first_bit = |key: &[u8]| shape_path_bit(&path_of(key), 0);
    let a = preimage(0);
    let b = (1..)
        .map(preimage)
        .find(|key| first_bit(key) != first_bit(&a))
        .unwrap();
    let c = (1..)
        .map(preimage)
        .find(|key| *key != b && first_bit(key) == first_bit(&a))
        .unwrap();

    let old_payload = workload::initial_payload(5, 0, 24);
    let (key_a, value_a) = (H::bytes(&a), H::bytes(&old_payload));
    let (key_b, value_b) = (H::bytes(&b), H::bytes(&workload::initial_payload(5, 1, 24)));
    let leaf_a = || Shape::Leaf(key_a, value_a);
    let leaf_b = || Shape::Leaf(key_b, value_b);
    let bit = first_bit(&a);
    let old_root = Shape::side(bit, leaf_a(), leaf_b()).root::<H>();

    let mut delta = vec![0u8; 24];
    delta[0] = 1;
    let update = vec![Rec {
        op: OP_UPDATE,
        hint: 0,
        key: a.clone(),
        arg: delta,
    }];
    let insert = vec![Rec {
        op: OP_INSERT,
        hint: 0,
        key: c,
        arg: workload::initial_payload(5, 2, 24),
    }];

    // The honest shapes pass, so the rejections below are about the shapes.
    run(
        arm,
        &Shape::side(bit, leaf_a(), leaf_b()).witness(old_root, &update, &old_payload),
    );
    run(
        arm,
        &Shape::side(bit, leaf_a(), leaf_b()).witness(old_root, &insert, &[]),
    );

    // A record cannot go into an opaque hash, even one holding the leaf's true hash.
    let opaque = Shape::Hash(<H as JmtHasher>::leaf(&key_a, &value_a));
    let witness = Shape::side(bit, opaque, leaf_b()).witness(old_root, &update, &old_payload);
    assert_rejected(arm, &witness, "opaque hash");

    // An empty slot cannot stand in for a real leaf, whatever the record claims about it.
    let witness = Shape::side(bit, Shape::Null, leaf_b()).witness(old_root, &update, &old_payload);
    assert_rejected(arm, &witness, "update of an absent key");
    let witness = Shape::side(bit, Shape::Null, leaf_b()).witness(old_root, &insert, &[]);
    assert_rejected(arm, &witness, "wrong old root");

    // A leaf opened as a branch cannot take the record further down.
    let opened = Shape::branch(Shape::Hash(key_a), Shape::Hash(value_a));
    let witness = Shape::side(bit, opened, leaf_b()).witness(old_root, &update, &old_payload);
    assert_rejected(arm, &witness, "opaque hash");

    // The deepest position is 256 levels down, which no real tree reaches.
    let path = path_of(&a);
    let at_depth = |depth: u32| {
        let mut node = leaf_a();
        for level in (0..depth).rev() {
            node = Shape::side(
                level < 256 && shape_path_bit(&path, level),
                node,
                Shape::Null,
            );
        }
        node
    };
    let deepest = at_depth(256);
    run(
        arm,
        &deepest.witness(deepest.root::<H>(), &update, &old_payload),
    );
    let too_deep = at_depth(257);
    let witness = too_deep.witness(too_deep.root::<H>(), &update, &old_payload);
    assert_rejected(arm, &witness, "path too long");
}

#[test]
fn jmt_batch_rejects_shapes_that_hide_or_invent_state() {
    jmt_batch_rejects_shapes::<Sha>(Arm::JmtBatchSha256);
    jmt_batch_rejects_shapes::<Pos>(Arm::JmtBatchPoseidon2);
}

fn imm_batch_rejects_records_out_of_place<H: ImmHasher>(arm: Arm) {
    const STRIDE: usize = 8 + 88 + 24;
    let swap_first_two = |witness: &mut [u8]| {
        let (first, second) = witness[HEADER_BYTES..].split_at_mut(STRIDE);
        first.swap_with_slice(&mut second[..STRIDE]);
    };
    let key = |index: u64| workload::key_preimage(9, 1000 + index, 84);
    let payload = |index: u64| workload::initial_payload(9, 1000 + index, 24);
    let insert = |index: u64| Rec {
        op: OP_INSERT,
        hint: 0,
        key: key(index),
        arg: payload(index),
    };

    // An update cannot come before the insert it builds on.
    let base = point(arm, Workload::Insert, 2, 50, 12, 1);
    let imm = ImmState::<H>::build(&base);
    let mut delta = vec![0u8; 24];
    delta[0] = 1;
    let records = vec![
        insert(0),
        Rec {
            op: OP_UPDATE,
            hint: 0,
            key: key(0),
            arg: delta,
        },
    ];
    let (mut witness, _) = batch_witness_of(&base, &imm, records).unwrap();
    assert!(!rejects(arm, &witness));
    swap_first_two(&mut witness);
    assert_rejected(arm, &witness, "out of bounds");

    // Leaves take positions in record order, so swapped inserts cannot reach the same state.
    let (mut witness, _) = batch_witness_of(&base, &imm, vec![insert(1), insert(2)]).unwrap();
    let original = run(arm, &witness);
    swap_first_two(&mut witness);
    if !rejects(arm, &witness) {
        let swapped = run(arm, &witness);
        assert_ne!(swapped.new_root, original.new_root);
        assert_ne!(swapped.batch_hash, original.batch_hash);
    }

    // One free slot: the first insert fits and a second one crosses the capacity mid-batch.
    let base = point(arm, Workload::Insert, 1, 14, 5, 1);
    let imm = ImmState::<H>::build(&base);
    let (good, _) = batch_witness_of(&base, &imm, vec![insert(3)]).unwrap();
    assert_eq!(run(arm, &good).new_len, 16);
    let (mut header, mut records, olds, mut engine) = regions(&good);
    let touched = u64::from_le_bytes(engine[8..16].try_into().unwrap());
    records.extend_from_slice(&workload::encode_records(
        &[Rec {
            hint: touched,
            ..insert(4)
        }],
        84,
        24,
    ));
    header.records = 2;
    engine[..8].copy_from_slice(&2u64.to_le_bytes());
    let (witness, _) = merkle_bench::prepare::assemble(header, &records, &olds, &engine);
    assert_rejected(arm, &witness, "tree full");
}

#[test]
fn imm_batch_rejects_records_out_of_place_with_both_hashes() {
    imm_batch_rejects_records_out_of_place::<Sha>(Arm::ImmBatchSha256);
    imm_batch_rejects_records_out_of_place::<Pos>(Arm::ImmBatchPoseidon2);
}

/// Records against whatever earlier batches left: some change keys they added or changed.
fn next_batch(known: &mut Vec<Vec<u8>>, state: &mut u64, k: usize) -> Vec<Rec> {
    let mut next = || {
        *state = state
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .wrapping_add(0x1234_5678_9abc_def1);
        *state ^ (*state >> 29)
    };
    let mut touched = std::collections::HashSet::new();
    (0..k)
        .map(|_| {
            if next() % 10 < 7 && !known.is_empty() {
                let key = known[next() as usize % known.len()].clone();
                touched.insert(key.clone());
                let mut arg = vec![0u8; 24];
                arg[..4].copy_from_slice(&(next() as u32).to_le_bytes());
                Rec {
                    op: OP_UPDATE,
                    hint: 0,
                    key,
                    arg,
                }
            } else {
                let mut key = vec![0u8; 84];
                key[..8].copy_from_slice(&next().to_le_bytes());
                key[8..16].copy_from_slice(&next().to_le_bytes());
                known.push(key.clone());
                let mut arg = vec![0u8; 24];
                arg[..8].copy_from_slice(&(next() >> 8).to_le_bytes());
                Rec {
                    op: OP_INSERT,
                    hint: 0,
                    key,
                    arg,
                }
            }
        })
        .collect()
}

fn imm_batches_chain<H: ImmHasher>(arm: Arm) {
    let base = point(arm, Workload::Mixed, 100, 0, 12, 1);
    let mut imm = ImmState::<H>::build(&base);
    let (mut known, mut random) = (Vec::new(), 11);
    let mut root = H::export(&imm.map.root());
    for _ in 0..20 {
        let records = next_batch(&mut known, &mut random, 100);
        let (witness, sidecar) = batch_witness_of(&base, &imm, records.clone()).unwrap();
        let output = run(arm, &witness);
        assert_eq!(output.old_root, root);
        assert_eq!(output.new_root, sidecar.expected_new_root.unwrap());
        root = output.new_root;
        imm.apply(&base, &records).unwrap();
        assert_eq!(H::export(&imm.map.root()), root);
    }
}

fn jmt_batches_chain<H: Hasher + JmtHasher, S: SimpleHasher>(stock: Arm, batch: Arm) {
    let base = point(batch, Workload::Mixed, 100, 0, 0, 1);
    let mut tree = State::<S>::build(7, 0, 84, 24).unwrap();
    let (mut known, mut random) = (Vec::new(), 11);
    let mut root = tree.root.0;
    for _ in 0..20 {
        let records = next_batch(&mut known, &mut random, 100);
        let (witness, sidecar) =
            jmt_state::batch_witness_of::<H, S>(&base, &tree, &records).unwrap();
        let output = run(batch, &witness);
        assert_eq!(output.old_root, root);
        assert_eq!(output.new_root, sidecar.expected_new_root.unwrap());
        let (witness, _) = jmt_state::stock_witness_of::<H, S>(&base, &tree, &records).unwrap();
        assert_eq!(run(stock, &witness).new_root, output.new_root);
        root = output.new_root;
        tree.apply(&records).unwrap();
        assert_eq!(tree.root.0, root);
    }
}

#[test]
fn consecutive_batches_chain_their_roots() {
    imm_batches_chain::<Sha>(Arm::ImmBatchSha256);
    imm_batches_chain::<Pos>(Arm::ImmBatchPoseidon2);
    jmt_batches_chain::<Sha, sha2::Sha256>(Arm::JmtStockSha256, Arm::JmtBatchSha256);
    jmt_batches_chain::<Pos, PoseidonNode<Native>>(
        Arm::JmtStockPoseidon2Node,
        Arm::JmtBatchPoseidon2,
    );
}

#[test]
fn workloads_are_deterministic_and_shaped_as_labelled() {
    let mixed = point(Arm::JmtBatchSha256, Workload::Mixed, 1000, 10_000, 0, 1);
    let records = workload::records(&mixed);
    assert_eq!(records.len(), 1000);
    let inserts = records
        .iter()
        .filter(|record| record.op == OP_INSERT)
        .count();
    assert!((120..=280).contains(&inserts), "{inserts} inserts");
    let distinct: std::collections::HashSet<_> =
        records.iter().map(|record| record.key.clone()).collect();
    assert!(distinct.len() < records.len(), "hot keys repeat");
    assert_eq!(
        workload::records(&mixed)[17].key,
        records[17].key,
        "same seed, same records"
    );
}
