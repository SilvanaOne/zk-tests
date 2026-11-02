//! Each guest on the zkVM's executor: it must do exactly what its engine does on the host, and fail
//! when a witness is tampered with.

use merkle_bench::{
    arm::Arm,
    measure::{aligned, as_bytes},
    point::{Point, RunMode, Workload},
    prepare::witness_for,
    software::take_counts,
};
use merkle_map::record::Header;
use sp1_core_executor::SyscallCode;
use sp1_sdk::{LightProver, ProveRequest, Prover, ProverClient, ProvingKey, SP1Stdin};

/// SP1 hashes the 152 bytes of public values with SHA-256 inside the guest: three blocks.
const PUBLIC_VALUE_BLOCKS: u64 = 3;

fn point(arm: Arm, workload: Workload, k: u32) -> Point {
    Point {
        arm,
        workload,
        k,
        sub_batches: 1,
        state: 60,
        height: if arm.is_jmt() { 0 } else { 10 },
        key_size: if workload == Workload::Append { 8 } else { 84 },
        value_size: 24,
        read_proofs: false,
        external_batch_hash: false,
        stop_after: 0,
        mode: RunMode::Execute,
        seed: 3,
    }
}

fn stdin(witness: &[u8]) -> SP1Stdin {
    let mut stdin = SP1Stdin::new();
    stdin.write_slice(witness);
    stdin
}

/// Whether a guest fails on a witness. A panic in a guest is an execution error or a failed exit.
async fn guest_rejects(light: &LightProver, arm: Arm, witness: &[u8]) -> bool {
    match light.execute(arm.elf(), stdin(witness)).await {
        Ok((_, report)) => report.exit_code != 0,
        Err(_) => true,
    }
}

#[tokio::test]
async fn every_guest_matches_its_engine_and_rejects_tampering() {
    let light = LightProver::new().await;
    for arm in Arm::ALL {
        for workload in [Workload::Insert, Workload::Update, Workload::Mixed] {
            let this = point(arm, workload, 12);
            let (witness, _) = witness_for(&this).unwrap();
            let words = aligned(&witness);

            take_counts();
            let expected = arm.native(as_bytes(&words));
            let counts = take_counts();

            let (public_values, report) = light.execute(arm.elf(), stdin(&witness)).await.unwrap();
            assert_eq!(report.exit_code, 0, "{}", this.run_id());
            assert_eq!(
                public_values.as_slice(),
                expected.as_bytes(),
                "{}",
                this.run_id()
            );
            assert_eq!(
                report.syscall_counts[SyscallCode::SHA_COMPRESS],
                counts.sha_compress + PUBLIC_VALUE_BLOCKS,
                "{}",
                this.run_id()
            );
            assert_eq!(
                report.syscall_counts[SyscallCode::SHA_EXTEND],
                counts.sha_extend + PUBLIC_VALUE_BLOCKS,
                "{}",
                this.run_id()
            );
            assert_eq!(
                report.syscall_counts[SyscallCode::POSEIDON2],
                counts.poseidon2,
                "{}",
                this.run_id()
            );

            let mut wrong_root = witness.clone();
            wrong_root[core::mem::offset_of!(Header, old_root)] ^= 1;
            assert!(
                guest_rejects(&light, arm, &wrong_root).await,
                "{} accepted a wrong old root",
                this.run_id()
            );
            let mut truncated = witness.clone();
            truncated.truncate(witness.len() - 8);
            assert!(
                guest_rejects(&light, arm, &truncated).await,
                "{} accepted a truncated witness",
                this.run_id()
            );
            let mut tampered = witness.clone();
            let last = witness.len() - 12;
            tampered[last] ^= 1;
            assert!(
                guest_rejects(&light, arm, &tampered).await,
                "{} accepted a changed witness",
                this.run_id()
            );
        }
    }
}

/// A stock guest must not accept the proof of another key's leaf, however the arithmetic falls.
#[tokio::test]
async fn stock_guests_reject_a_key_outside_the_proved_leaf() {
    use jmt::{JellyfishMerkleTree, KeyHash};
    use merkle_bench::jmt_state::{Store, frame_update_proof};
    use merkle_map::{
        hash::{Hasher, JmtHasher, Sha256Raw},
        record::OP_INSERT,
    };
    type Sha = Sha256Raw<merkle_bench::software::Native>;

    // Keys whose preimages are chosen freely are hashes, so build the two-leaf tree from hashes and
    // pick a third key that falls outside the first leaf's subtree.
    let preimage = |seed: u64| -> Vec<u8> {
        let mut bytes = vec![0u8; 84];
        bytes[..8].copy_from_slice(&seed.to_le_bytes());
        bytes
    };
    let keys: Vec<[u8; 32]> = (0..64)
        .map(|seed| Sha::export(&Sha::bytes(&preimage(seed))))
        .collect();
    let first_bit = |key: &[u8; 32]| key[0] >> 7;
    let a = (0..64).find(|i| first_bit(&keys[*i]) == 0).unwrap();
    let b = (0..64).find(|i| first_bit(&keys[*i]) == 1).unwrap();
    let c = ((b + 1)..64).find(|i| first_bit(&keys[*i]) == 1).unwrap();

    let mut store = Store::default();
    let value = vec![1u8; 24];
    let (roots, batch) = JellyfishMerkleTree::<_, sha2::Sha256>::new(&store)
        .batch_put_value_sets(
            vec![vec![
                (KeyHash(keys[a]), value.clone()),
                (KeyHash(keys[b]), value.clone()),
            ]],
            None,
            0,
        )
        .unwrap();
    store.apply(&batch);
    let tree = JellyfishMerkleTree::<_, sha2::Sha256>::new(&store);

    // The real update proof for A, offered for an insert of C.
    let (_, proof, _) = tree
        .put_value_set_with_proof(vec![(KeyHash(keys[a]), Some(value.clone()))], 1)
        .unwrap();
    let mut engine = Vec::new();
    frame_update_proof(&borsh::to_vec(&proof).unwrap(), &mut engine);
    engine.resize(engine.len().next_multiple_of(8), 0);

    let mut records = Vec::new();
    records.extend_from_slice(&(OP_INSERT as u64).to_le_bytes());
    records.extend_from_slice(&preimage(c as u64));
    records.resize(8 + 88, 0);
    records.extend_from_slice(&[2u8; 24]);

    let header = Header {
        records: 1,
        key_size: 84,
        value_size: 24,
        sub_batches: 1,
        old_root: Sha::import(&roots[0].0),
        // The root that follows from putting C on A's siblings, so only the guard can object.
        new_root: <Sha as JmtHasher>::internal(
            &<Sha as JmtHasher>::leaf(&Sha::import(&keys[b]), &Sha::bytes(&value)),
            &<Sha as JmtHasher>::leaf(&Sha::import(&keys[c]), &Sha::bytes(&[2u8; 24])),
        ),
        ..Default::default()
    };
    let (witness, _) = merkle_bench::prepare::assemble(header, &records, &[], &engine);

    let light = LightProver::new().await;
    assert!(guest_rejects(&light, Arm::JmtStockSha2, &witness).await);
    assert!(guest_rejects(&light, Arm::JmtStockSha256, &witness).await);
}

/// Memory that only a precompile writes passes execution and then gives a proof that fails
/// verification, so these guests have to be proved, not only executed. See `ShaBackend::schedule`.
/// Takes a few minutes.
#[tokio::test]
#[ignore = "runs the real prover"]
async fn guests_with_raw_sha256_calls_prove_and_verify() {
    let client = ProverClient::from_env().await;
    for (arm, external_batch_hash) in [
        (Arm::ImmChainedSha256, false),
        (Arm::ImmBatchSha256, false),
        (Arm::JmtStockSha256, false),
        (Arm::JmtBatchSha256, false),
        (Arm::ImmBatchPoseidon2, true),
    ] {
        let this = Point {
            external_batch_hash,
            ..point(arm, Workload::Mixed, 4)
        };
        let (witness, _) = witness_for(&this).unwrap();
        let pk = client.setup(arm.elf()).await.unwrap();
        let proof = client.prove(&pk, stdin(&witness)).core().await.unwrap();
        client
            .verify(&proof, pk.verifying_key(), None)
            .unwrap_or_else(|error| panic!("{}: {error}", this.run_id()));
        println!("{} proved and verified", this.run_id());
    }
}

/// The executor reduces a non-canonical element silently, but the proof constrains its range. So
/// such a witness passes execution and must not produce a proof that verifies. The prover has not
/// been seen to finish on one, so this waits out the limit: ten minutes.
#[tokio::test]
#[ignore = "runs the real prover"]
async fn non_canonical_input_passes_execution_and_fails_proving() {
    use merkle_map::imm::batch::{BlockHeader, TouchedLeaf};

    let arm = Arm::ImmBatchPoseidon2;
    let this = point(arm, Workload::Insert, 1);
    let (mut witness, _) = witness_for(&this).unwrap();
    let words = aligned(&witness);
    // SAFETY: the buffer starts with a header.
    let header: Header = unsafe { as_bytes(&words).as_ptr().cast::<Header>().read() };
    let engine = size_of::<Header>() + (header.records_bytes + header.olds_bytes) as usize;

    // Add the modulus to one element of the touched leaf's value: the same field element, in a form
    // that is out of range.
    let value = engine + size_of::<BlockHeader>() + core::mem::offset_of!(TouchedLeaf, leaf) + 32;
    let element = u32::from_le_bytes(witness[value..value + 4].try_into().unwrap());
    let raised = element
        .checked_add(0x7f00_0001)
        .expect("element below 2^31");
    witness[value..value + 4].copy_from_slice(&raised.to_le_bytes());

    let light = LightProver::new().await;
    let (_, report) = light.execute(arm.elf(), stdin(&witness)).await.unwrap();
    assert_eq!(report.exit_code, 0, "execution should not notice");

    let client = ProverClient::from_env().await;
    let pk = client.setup(arm.elf()).await.unwrap();
    let outcome = tokio::time::timeout(
        std::time::Duration::from_secs(600),
        tokio::spawn(async move {
            let client = ProverClient::from_env().await;
            let proof = client.prove(&pk, stdin(&witness)).core().await;
            proof.is_ok_and(|proof| client.verify(&proof, pk.verifying_key(), None).is_ok())
        }),
    )
    .await;
    let verified = matches!(outcome, Ok(Ok(true)));
    assert!(
        !verified,
        "a non-canonical witness produced a proof that verifies"
    );
}
