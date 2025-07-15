import { Cache, verify, setNumberOfWorkers, Field, UInt32 } from "o1js";
import { test, describe } from "node:test";
import assert from "node:assert";
import {
  scalar,
  digestStruct,
  commit,
  update,
  CommitmentProgram,
} from "../src/commitment.js";

describe("commitment equivalence with Move", () => {
  test("digest and update matches Move result", () => {
    console.log("Testing commitment equivalence with Move...");

    // two structs, each 2 fields
    const struct1 = [scalar(1n), scalar(2n)];
    const struct2 = [scalar(3n), scalar(4n)];

    const c0 = digestStruct(struct1);
    const c1 = digestStruct(struct2);
    const commit0 = commit([c0, c1]);

    const expectedCommit0 =
      "0x67c9d15a9ca6783200ae2bc0fe16bac805cf4abb8f79452334fd264669260e4b";
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
    const commit1 = update(
      commit0,
      oldStructDigest,
      newStructDigest,
      UInt32.from(0n)
    );

    // recompute ground‑truth
    const c0new = newStructDigest; // We already computed this above
    const commitTruth = commit([c0new, c1]);

    const expectedCommit1 =
      "0x67c9d15a9ca6783200ae2bc0fe16bac805cf4abb8f79452334fd264669260e50";
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
      UInt32.zero
    );

    setNumberOfWorkers(8); // Mac M2 Max

    const cache = Cache.FileSystem("./cache");

    console.log("\ncompiling...");
    console.time("compile");
    const vk = (await CommitmentProgram.compile({ cache })).verificationKey;
    console.timeEnd("compile");
    console.log("vk:", vk.hash.toJSON()); // 12468479725166761494505226643698891732004953180564018405267393178782471181991

    console.log("\n=== TESTING UPDATE METHOD ===");
    console.log("proving update...");
    console.time("prove update");
    const zkUpdateResult = await CommitmentProgram.updateCommitment(
      commit0,
      oldStructDigest,
      newStructDigest,
      UInt32.from(0n)
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
