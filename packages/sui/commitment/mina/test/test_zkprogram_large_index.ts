import {
  ZkProgram,
  Provable,
  Cache,
  verify,
  setNumberOfWorkers,
  Field,
  UInt32,
} from "o1js";
import {
  scalar,
  digestStruct,
  commit,
  update,
  CommitmentProgram,
  type CanonicalElement,
  type AlmostReducedElement,
} from "../src/commitment.js";

async function testLargeIndexZkProgram() {
  console.log("Testing ZkProgram updateCommitment with large indices...");

  // Create a large table with many elements
  const tableSize = 1000;
  const structs: CanonicalElement[][] = [];
  const digests: AlmostReducedElement[] = [];

  // Generate test data
  for (let i = 0; i < tableSize; i++) {
    const struct = [scalar(BigInt(i * 2 + 1)), scalar(BigInt(i * 2 + 2))];
    structs.push(struct);
    digests.push(digestStruct(struct));
  }

  // Initial commitment
  const initialCommitment = commit(digests);
  console.log(`Initial commitment for ${tableSize} elements computed`);

  // Test indices to verify
  const testIndices = [0, 1, 10, 100, 500, 999];

  setNumberOfWorkers(8);
  const cache = Cache.FileSystem("./cache");

  console.log("Compiling ZkProgram...");
  console.time("compile");
  const vk = (await CommitmentProgram.compile({ cache })).verificationKey;
  console.timeEnd("compile");

  for (const testIndex of testIndices) {
    console.log(`\nTesting index ${testIndex}...`);

    // Update: change the struct at testIndex
    const oldStruct = structs[testIndex];
    const newStruct = [scalar(BigInt(9999)), scalar(BigInt(8888))];

    const oldStructDigest = digests[testIndex];
    const newStructDigest = digestStruct(newStruct);

    // Direct calculation
    console.time(`direct update index ${testIndex}`);
    const directResult = update(
      initialCommitment,
      oldStructDigest,
      newStructDigest,
      UInt32.from(BigInt(testIndex))
    );
    console.timeEnd(`direct update index ${testIndex}`);

    // ZkProgram calculation
    console.time(`zkprogram prove index ${testIndex}`);
    const zkResult = await CommitmentProgram.updateCommitment(
      initialCommitment,
      oldStructDigest,
      newStructDigest,
      UInt32.from(BigInt(testIndex))
    );
    console.timeEnd(`zkprogram prove index ${testIndex}`);

    // Verify proof
    console.time(`zkprogram verify index ${testIndex}`);
    const verified = await verify(zkResult.proof, vk);
    console.timeEnd(`zkprogram verify index ${testIndex}`);

    // Check results match
    const directHex =
      "0x" + directResult.toBigInt().toString(16).padStart(64, "0");
    const zkHex =
      "0x" +
      zkResult.proof.publicOutput.toBigInt().toString(16).padStart(64, "0");

    console.log(`  Direct result:    ${directHex}`);
    console.log(`  ZkProgram result: ${zkHex}`);
    console.log(`  Proof verified:   ${verified}`);

    if (directHex === zkHex && verified) {
      console.log(`  ✅ Index ${testIndex} PASSED`);
    } else {
      console.log(`  ❌ Index ${testIndex} FAILED`);
      throw new Error(`Test failed for index ${testIndex}`);
    }
  }

  console.log(
    "\n🎉 All large index tests passed! ZkProgram updateCommitment works correctly for any index."
  );
}

// Run the test
testLargeIndexZkProgram().catch(console.error);
