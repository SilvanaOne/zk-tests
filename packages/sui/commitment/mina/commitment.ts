/// <reference types="node" />
import {
  createForeignField,
  ZkProgram,
  Provable,
  Cache,
  verify,
  setNumberOfWorkers,
  Field,
} from "o1js";
import { test, describe } from "node:test";
import assert from "node:assert";

// BLS12‑381 scalar field prime
const BLS_FR =
  0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001n;

const Fr = createForeignField(BLS_FR);

// Use the proper types from the foreign field system
type CanonicalElement = InstanceType<typeof Fr.Canonical>;
type AlmostReducedElement = InstanceType<typeof Fr.AlmostReduced>;

// ----- constants taken from Move code (big‑endian hex) -----
const S: CanonicalElement =
  Fr.from(0x1582695da6689f26db7bb3eb32907ecd0ac3af032aefad31a069352705f0d459n);
const R: CanonicalElement =
  Fr.from(0x149fa8c209ab655fd480a3aff7d16dc72b6a3943e4b95fcf7909f42d9c17a552n);

// ----- helpers -----
function scalar(n: bigint): CanonicalElement {
  return Fr.from(n);
}

// inner: digest one struct
function digestStruct(fields: CanonicalElement[]): AlmostReducedElement {
  let d: AlmostReducedElement = Fr.from(0n).assertAlmostReduced();
  for (const f of fields) {
    const prod = d.mul(S); // returns Unreduced
    d = prod.add(f).assertAlmostReduced(); // reduce for next iteration
  }
  return d;
}

// outer: commit whole table (vector of digests)
function commit(table: AlmostReducedElement[]): AlmostReducedElement {
  let acc: AlmostReducedElement = Fr.from(0n).assertAlmostReduced();
  for (const c of table) {
    const prod = acc.mul(R); // returns Unreduced
    acc = prod.add(c).assertAlmostReduced(); // reduce for next iteration
  }
  return acc;
}

// Helper function to compute base^exp for foreign field elements
function scalarPow(base: CanonicalElement, exp: number): CanonicalElement {
  let acc = Fr.from(1n);
  for (let i = 0; i < exp; i++) {
    acc = acc.mul(base).assertCanonical();
  }
  return acc;
}

// constant‑time single‑field update using struct digest recalculation
function update(
  oldTableCommitment: AlmostReducedElement,
  oldStructDigest: AlmostReducedElement,
  newStructDigest: AlmostReducedElement,
  index: number,
  tableSize: number
): AlmostReducedElement {
  // The table commitment formula in commit() is:
  // acc = prod.add(c).assertAlmostReduced() where prod = acc.mul(R)
  // For table [t0, t1, t2, ...] this produces: t0*R^(n-1) + t1*R^(n-2) + ... + t(n-1)*R^0
  // So position i has coefficient R^(table_length - 1 - i)

  // Calculate the coefficient for this position
  const coefficientPower = tableSize - 1 - index;
  const rPowI = scalarPow(R, coefficientPower);

  // Calculate the change: new_commitment = old_commitment + (new_struct - old_struct) * R^coeff
  const structDelta = newStructDigest
    .sub(oldStructDigest)
    .assertAlmostReduced();
  const tableDelta = structDelta.mul(rPowI).assertAlmostReduced();
  return oldTableCommitment.add(tableDelta).assertAlmostReduced();
}

// Create ZkProgram for commitment update
const CommitmentProgram = ZkProgram({
  name: "CommitmentUpdate",
  publicOutput: Fr.AlmostReduced.provable,
  methods: {
    updateCommitment: {
      privateInputs: [
        Fr.AlmostReduced.provable, // oldTableCommitment
        Fr.AlmostReduced.provable, // oldStructDigest
        Fr.AlmostReduced.provable, // newStructDigest
        Field, // index
      ],
      async method(
        oldTableCommitment: AlmostReducedElement,
        oldStructDigest: AlmostReducedElement,
        newStructDigest: AlmostReducedElement,
        index: Field
      ) {
        // Calculate R^coefficientPower
        // For simplicity, we'll handle the specific case of a 2-element table (tableSize = 2)
        // where position 0 has coefficient R^1 = R and position 1 has coefficient R^0 = 1
        const isPosition0 = index.equals(Field(0));
        const rPowI = Provable.if(
          isPosition0,
          Fr.Canonical.provable,
          R.assertCanonical(),
          Fr.from(1n)
        );

        // Calculate the change: new_commitment = old_commitment + (new_struct - old_struct) * R^coeff
        const structDelta = newStructDigest
          .sub(oldStructDigest)
          .assertAlmostReduced();
        const tableDelta = structDelta.mul(rPowI).assertAlmostReduced();
        const newCommitment = oldTableCommitment
          .add(tableDelta)
          .assertAlmostReduced();

        return { publicOutput: newCommitment };
      },
    },
    commitFromScratch: {
      privateInputs: [
        Provable.Array(Fr.Canonical.provable, 2), // struct1 [field0, field1]
        Provable.Array(Fr.Canonical.provable, 2), // struct2 [field0, field1]
      ],
      async method(struct1: CanonicalElement[], struct2: CanonicalElement[]) {
        // Digest each struct
        const c0 = digestStruct(struct1);
        const c1 = digestStruct(struct2);

        // Commit the table of digests
        const commitment = commit([c0, c1]);

        return { publicOutput: commitment };
      },
    },
  },
});

describe("commitment equivalence with Move", () => {
  test("digest‑and‑update matches Move result", () => {
    console.log("Testing commitment equivalence with Move...");

    // two structs, each 2 fields
    const struct1 = [scalar(1n), scalar(2n)];
    const struct2 = [scalar(3n), scalar(4n)];

    const c0 = digestStruct(struct1);
    const c1 = digestStruct(struct2);
    const commit0 = commit([c0, c1]);

    const expectedCommit0 =
      "0x69b424994bc0beb82c845b7e2b69d994e640671ee5787a1a6373929df04953eb";
    const actualCommit0 =
      "0x" + commit0.toBigInt().toString(16).padStart(64, "0");
    console.log("commit0:", actualCommit0);
    assert.strictEqual(
      actualCommit0,
      expectedCommit0,
      "commit0 should match Move result"
    );

    // update: field1 of struct0 becomes 7 (struct update approach)
    // Create the new struct with updated field
    const struct1Updated = [scalar(1n), scalar(7n)];

    // Calculate old and new struct digests
    const oldStructDigest = c0; // We already computed this as digestStruct(struct1)
    const newStructDigest = digestStruct(struct1Updated);

    // Update struct at index 0 in a 2-element table
    const commit1 = update(commit0, oldStructDigest, newStructDigest, 0, 2);

    // recompute ground‑truth
    const c0new = newStructDigest; // We already computed this above
    const commitTruth = commit([c0new, c1]);

    const expectedCommit1 =
      "0x5ce4c910527c3c4f1fcdb5e5f8df26736b95e16f5d18fd28c0a55782fcbf8e84";
    const actualCommit1 =
      "0x" + commit1.toBigInt().toString(16).padStart(64, "0");
    const actualCommitTruth =
      "0x" + commitTruth.toBigInt().toString(16).padStart(64, "0");

    console.log("commit1:", actualCommit1);
    console.log("commitTruth:", actualCommitTruth);

    // Check equality with Move test results
    assert.strictEqual(
      actualCommit1,
      expectedCommit1,
      "commit1 should match Move result"
    );
    assert.strictEqual(
      actualCommitTruth,
      expectedCommit1,
      "commitTruth should match Move result"
    );

    // Check that update produces same result as recomputation
    assert.strictEqual(
      actualCommit1,
      actualCommitTruth,
      "commit1 should equal commitTruth"
    );

    console.log("✅ Test passed! Commitments match Move results exactly.");
  });

  test("ZkProgram commitment update constraint analysis", async () => {
    console.log(
      "\nAnalyzing ZkProgram constraint count for commitment update..."
    );

    // Analyze the constraint count for both methods
    const methods = await CommitmentProgram.analyzeMethods();
    const updateMethodStats = (methods as any).updateCommitment;
    const commitFromScratchStats = (methods as any).commitFromScratch;

    console.log(`\n=== UPDATE METHOD ===`);
    console.log(`Commitment update constraints: ${updateMethodStats.rows}`);
    console.log(`Gates breakdown:`);
    console.log(`  - Total gates: ${updateMethodStats.gates.length}`);

    // Safely analyze gate types for update method
    const updateGateTypes = new Map<string, number>();
    for (const gate of updateMethodStats.gates) {
      const typ = gate?.typ || gate?.type || "Unknown";
      updateGateTypes.set(typ, (updateGateTypes.get(typ) || 0) + 1);
    }

    console.log(`  - Gate types breakdown:`);
    for (const [type, count] of updateGateTypes.entries()) {
      console.log(`    * ${type}: ${count}`);
    }

    console.log(`\n=== COMMIT FROM SCRATCH METHOD ===`);
    console.log(
      `Commit from scratch constraints: ${commitFromScratchStats.rows}`
    );
    console.log(`Gates breakdown:`);
    console.log(`  - Total gates: ${commitFromScratchStats.gates.length}`);

    // Safely analyze gate types for commit from scratch method
    const commitGateTypes = new Map<string, number>();
    for (const gate of commitFromScratchStats.gates) {
      const typ = gate?.typ || gate?.type || "Unknown";
      commitGateTypes.set(typ, (commitGateTypes.get(typ) || 0) + 1);
    }

    console.log(`  - Gate types breakdown:`);
    for (const [type, count] of commitGateTypes.entries()) {
      console.log(`    * ${type}: ${count}`);
    }

    // Test that the ZkProgram produces the same result as direct calculation
    const struct1 = [scalar(1n), scalar(2n)];
    const struct2 = [scalar(3n), scalar(4n)];
    const c0 = digestStruct(struct1);
    const c1 = digestStruct(struct2);
    const commit0 = commit([c0, c1]);

    // Prepare for update: field1 of struct0 becomes 7
    const struct1Updated = [scalar(1n), scalar(7n)];
    const oldStructDigest = c0; // We already computed this as digestStruct(struct1)
    const newStructDigest = digestStruct(struct1Updated);

    // Direct calculation
    const directResult = update(
      commit0,
      oldStructDigest,
      newStructDigest,
      0,
      2
    );

    setNumberOfWorkers(8); // Mac M2 Max

    const cache = Cache.FileSystem("./cache");

    console.log("\ncompiling...");
    console.time("compile");
    const vk = (await CommitmentProgram.compile({ cache })).verificationKey;
    console.timeEnd("compile");

    console.log("\n=== TESTING UPDATE METHOD ===");
    console.log("proving update...");
    console.time("prove update");
    const zkUpdateResult = await CommitmentProgram.updateCommitment(
      commit0,
      oldStructDigest,
      newStructDigest,
      Field(0)
    );
    console.timeEnd("prove update");

    console.log("verifying update...");
    console.time("verify update");
    const updateVerified = await verify(zkUpdateResult.proof, vk);
    console.timeEnd("verify update");
    console.log("update verified:", updateVerified);
    assert(updateVerified, "Update ZkProgram result should be verified");

    const updateProvedResult = zkUpdateResult.proof.publicOutput;
    console.log(
      `Update ZkProgram result: 0x${updateProvedResult
        .toBigInt()
        .toString(16)
        .padStart(64, "0")}`
    );

    // Compare the update results
    assert(
      updateProvedResult.toBigInt() === directResult.toBigInt(),
      "Update ZkProgram result should match direct calculation"
    );

    console.log(`✅ Update ZkProgram result matches direct calculation!`);

    console.log("\n=== TESTING COMMIT FROM SCRATCH METHOD ===");
    console.log("proving commit from scratch...");
    console.time("prove commit");
    const zkCommitResult = await CommitmentProgram.commitFromScratch(
      struct1,
      struct2
    );
    console.timeEnd("prove commit");

    console.log("verifying commit...");
    console.time("verify commit");
    const commitVerified = await verify(zkCommitResult.proof, vk);
    console.timeEnd("verify commit");
    console.log("commit verified:", commitVerified);
    assert(commitVerified, "Commit ZkProgram result should be verified");

    const commitProvedResult = zkCommitResult.proof.publicOutput;
    console.log(
      `Commit ZkProgram result: 0x${commitProvedResult
        .toBigInt()
        .toString(16)
        .padStart(64, "0")}`
    );

    // Compare the commit results
    assert(
      commitProvedResult.toBigInt() === commit0.toBigInt(),
      "Commit ZkProgram result should match direct calculation"
    );

    console.log(`✅ Commit ZkProgram result matches direct calculation!`);

    console.log(`\n=== ANALYSIS SUMMARY ===`);
    console.log(`Update method constraints: ${updateMethodStats.rows}`);
    console.log(
      `Commit from scratch constraints: ${commitFromScratchStats.rows}`
    );

    // Verify the constraint count is reasonable (foreign field operations are expensive but should be manageable)
    assert(updateMethodStats.rows > 0, "Update should have some constraints");
    assert(
      commitFromScratchStats.rows > 0,
      "Commit should have some constraints"
    );
    assert(
      updateMethodStats.rows < 1000,
      "Update constraint count should be reasonable (< 1000)"
    );
    assert(
      commitFromScratchStats.rows < 1000,
      "Commit constraint count should be reasonable (< 1000)"
    );

    console.log(
      `✅ Constraint analysis passed! Both methods use reasonable constraint counts.`
    );
  });
});
