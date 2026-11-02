//! The unchanged `jmt` crate's update proof, one proof per key.
//!
//! The guest derives its own writes, sorted by key with repeated writes to one key collapsed, so the
//! host's write set is never used. Engine region: one length-framed Borsh proof per written key, in
//! key order, and then, with `FLAG_READ_PROOFS`, one more per key against the old root.

use jmt::{
    KeyHash, RootHash, SimpleHasher,
    proof::{SparseMerkleProof, UpdateMerkleProof},
};

use crate::{
    hash::{Digest, Hasher},
    jellyfish::guard,
    record::{
        FLAG_READ_PROOFS, OP_INSERT, OP_UPDATE, PublicValues, Record, STOP_AFTER_DECODE,
        STOP_AFTER_DERIVE, TREE_JMT, Witness, apply_update, batch_hash_bytes, batch_seed,
        batch_step, state_version,
    },
};

struct Op<'a> {
    key: [u8; 32],
    sequence: u32,
    record: Record<'a>,
}

/// Length-framed proofs.
struct Frames<'a>(&'a [u8]);

impl<'a> Frames<'a> {
    fn next(&mut self) -> &'a [u8] {
        let (length, rest) = self.0.split_first_chunk::<4>().expect("missing proof");
        let (frame, rest) = rest.split_at(u32::from_le_bytes(*length) as usize);
        self.0 = rest;
        frame
    }
}

pub fn run<H: Hasher, S: SimpleHasher>(bytes: &[u8]) -> PublicValues {
    let witness = Witness::parse(bytes);
    let header = witness.header;
    let version = state_version(TREE_JMT, H::ID);
    let old_root = H::export(&header.old_root);
    let new_root = H::export(&header.new_root);
    let mut output = PublicValues::start(header, version, old_root);

    // Derive keys and commit to the records in their given order.
    let mut batch_hash: Digest = batch_seed(version, header.records);
    let mut ops = Vec::with_capacity(header.records as usize);
    for (sequence, record) in witness.records().enumerate() {
        let key = S::hash(record.key);
        batch_hash = batch_step::<H>(
            header.flags,
            &batch_hash,
            &record,
            &H::import(&key),
            &H::bytes(record.arg),
        );
        ops.push(Op {
            key,
            sequence: sequence as u32,
            record,
        });
    }
    output.batch_hash = batch_hash_bytes::<H>(header.flags, &batch_hash);
    if header.stop_after == STOP_AFTER_DERIVE {
        return output;
    }

    // Collapse to one write per key, in key order, and check each proof against what it must show.
    ops.sort_unstable_by(|a, b| a.key.cmp(&b.key).then(a.sequence.cmp(&b.sequence)));
    let mut frames = Frames(witness.engine);
    let mut olds = witness.olds();
    let mut proofs = Vec::with_capacity(ops.len());
    let mut writes: Vec<(KeyHash, Option<Vec<u8>>)> = Vec::with_capacity(ops.len());
    let mut reads: Vec<(KeyHash, Option<Vec<u8>>)> = Vec::new();
    let read_proofs = header.flags & FLAG_READ_PROOFS != 0;
    for run in ops.chunk_by(|a, b| a.key == b.key) {
        let key = run[0].key;
        let frame = frames.next();
        let (mut payload, rest) = match run[0].record.op {
            OP_INSERT => {
                guard::check_insert(frame, &key);
                if read_proofs {
                    reads.push((KeyHash(key), None));
                }
                (run[0].record.arg.to_vec(), &run[1..])
            }
            OP_UPDATE => {
                let old = olds.next().expect("missing old payload");
                let old = &old[..header.value_size as usize];
                guard::check_update(frame, &key, &S::hash(old));
                if read_proofs {
                    reads.push((KeyHash(key), Some(old.to_vec())));
                }
                (old.to_vec(), run)
            }
            op => panic!("bad operation {op}"),
        };
        for op in rest {
            assert_eq!(op.record.op, OP_UPDATE, "insert of an existing key");
            apply_update(&mut payload, op.record.arg);
        }
        proofs.push(borsh::from_slice::<SparseMerkleProof<S>>(frame).expect("bad proof"));
        writes.push((KeyHash(key), Some(payload)));
    }
    if read_proofs {
        for (key, value) in &reads {
            let proof =
                borsh::from_slice::<SparseMerkleProof<S>>(frames.next()).expect("bad read proof");
            proof
                .verify(RootHash(old_root), *key, value.as_ref())
                .expect("bad read proof");
        }
    }
    // The region ends on an 8-byte boundary, so up to 7 zero bytes may follow the last proof.
    assert!(
        frames.0.len() < 8 && frames.0.iter().all(|byte| *byte == 0),
        "trailing proofs"
    );
    assert!(olds.next().is_none(), "trailing old payloads");
    if header.stop_after == STOP_AFTER_DECODE {
        return output;
    }

    UpdateMerkleProof::new(proofs)
        .verify_update(RootHash(old_root), RootHash(new_root), writes)
        .expect("bad update proof");
    // Only now is the claimed root more than a claim.
    output.new_root = new_root;
    output
}
