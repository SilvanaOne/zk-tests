//! A batch as one witness per record, each verified against the root the last one produced. This is
//! the per-record shape of the map's existing guest, extended to a batch of mixed records.

use borsh::BorshDeserialize;

use crate::{
    hash::{Digest, ImmHasher},
    imm::{provable::ProvableIndexedMerkleMap, types::RecordWitness},
    record::{
        FLAG_SEQUENTIAL, PublicValues, Record, STOP_AFTER_DECODE, STOP_AFTER_DERIVE, TREE_IMM,
        Witness, apply_update, batch_hash_bytes, batch_seed, batch_step, state_version,
    },
};

/// A record's key: a hash of its preimage, or for an append-only set, its sequence number.
#[inline(always)]
pub fn derive_key<H: ImmHasher>(flags: u32, record: &Record) -> Digest {
    if flags & FLAG_SEQUENTIAL != 0 {
        H::encode_sequence(u64::from_le_bytes(record.key.try_into().unwrap()))
    } else {
        H::bytes(record.key)
    }
}

pub fn run<H: ImmHasher>(bytes: &[u8]) -> PublicValues {
    let witness = Witness::parse(bytes);
    let header = witness.header;
    assert!((1..=63).contains(&header.height), "bad height");
    let depth = header.height as usize - 1;
    assert!(
        header.old_len >= 1 && header.old_len <= 1u64 << depth,
        "bad length"
    );
    assert!(
        H::bind(&header.old_internal, header.old_len) == header.old_root,
        "wrong old root"
    );

    let version = state_version(TREE_IMM, H::ID);
    let mut output = PublicValues {
        height: header.height,
        old_len: header.old_len,
        ..PublicValues::start(header, version, H::export(&header.old_root))
    };

    let mut batch_hash = batch_seed(version, header.records);
    let mut derived = Vec::with_capacity(header.records as usize);
    for record in witness.records() {
        let key = derive_key::<H>(header.flags, &record);
        let arg = H::bytes(record.arg);
        batch_hash = batch_step::<H>(header.flags, &batch_hash, &record, &key, &arg);
        derived.push((key, arg, record));
    }
    output.batch_hash = batch_hash_bytes::<H>(header.flags, &batch_hash);
    if header.stop_after == STOP_AFTER_DERIVE {
        return output;
    }

    let mut reader = witness.engine;
    let proofs = Vec::<RecordWitness>::deserialize(&mut reader).expect("bad witness");
    assert!(
        reader.len() < 8 && reader.iter().all(|byte| *byte == 0),
        "trailing witness"
    );
    assert_eq!(proofs.len(), derived.len(), "wrong witness count");
    if header.stop_after == STOP_AFTER_DECODE {
        return output;
    }

    let mut root = header.old_internal;
    let mut length = header.old_len;
    let mut olds = witness.olds();
    let value_size = header.value_size as usize;
    let mut payload = vec![0u8; value_size];
    for ((key, arg, record), proof) in derived.iter().zip(&proofs) {
        match proof {
            RecordWitness::Insert(proof) => {
                root = ProvableIndexedMerkleMap::<H>::insert(&root, length, proof, key, arg, depth)
                    .expect("bad insert");
                length += 1;
            }
            RecordWitness::Update(proof) => {
                let old = &olds.next().expect("missing old payload")[..value_size];
                let old_value = H::bytes(old);
                payload.copy_from_slice(old);
                apply_update(&mut payload, record.arg);
                let new_value = H::bytes(&payload);
                root = ProvableIndexedMerkleMap::<H>::update(
                    &root, proof, key, &old_value, &new_value, depth,
                )
                .expect("bad update");
            }
        }
        let expected = if matches!(proof, RecordWitness::Insert(_)) {
            crate::record::OP_INSERT
        } else {
            crate::record::OP_UPDATE
        };
        assert_eq!(record.op, expected, "witness for another operation");
    }
    assert!(olds.next().is_none(), "trailing old payloads");

    output.new_len = length;
    output.new_root = H::export(&H::bind(&root, length));
    output
}
