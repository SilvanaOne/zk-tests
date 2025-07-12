import { test, describe } from "node:test";
import assert from "node:assert";
import { TABLE0 } from "../src/constants.js";
import { createMerkleTree, Witness } from "../src/tree.js";
import { Field, MerkleTree } from "o1js";
import { serializeFields, deserializeFields } from "@silvana-one/mina-utils";
const fs = await import("node:fs/promises");
const path = await import("node:path");

describe("Merkle Tree with TABLE0", () => {
  test("should create a merkle tree from TABLE0 constants", async () => {
    // Convert TABLE0 to regular bigint array (remove readonly)
    const table0Array = Array.from(TABLE0);

    // Create the merkle tree
    const tree = createMerkleTree(table0Array);

    // Verify the tree was created successfully
    assert(tree, "Tree should be defined");
    assert.strictEqual(tree.leafCount, 1024n, "Tree should have 1024 leaves");

    // Verify we can get the root
    const root = tree.getRoot();
    assert(root, "Root should be defined");

    // Verify some leaf values by checking a few entries
    const leaf0 = tree.getLeaf(0n);
    assert.strictEqual(leaf0.toBigInt(), TABLE0[0]);
    const leaf1 = tree.getLeaf(1n);
    assert.strictEqual(leaf1.toBigInt(), TABLE0[1]);
    const leaf1023 = tree.getLeaf(1023n);
    assert.strictEqual(leaf1023.toBigInt(), TABLE0[1023]);
    const leafIndex = 10;
    const leaf = tree.getLeaf(BigInt(leafIndex));
    const witness = new Witness(tree.getWitness(10n));
    const serializedWitness = serializeFields(witness.toFields());
    console.log("serializedWitness:", serializedWitness);
    const restoredWitness = Witness.fromFields(
      deserializeFields(serializedWitness)
    );
    const witnessRoot = restoredWitness.calculateRoot(leaf);
    const witnessIndex = restoredWitness.calculateIndex();
    assert(
      witnessIndex.equals(BigInt(leafIndex)),
      "Witness index should match leaf index"
    );
    assert(witnessRoot.equals(root), "Witness root should match tree root");
    console.log(`   Root: ${root.toString()}`);
    const witnesses: string[] = [];
    for (let i = 0; i < 1024; i++) {
      const witness = new Witness(tree.getWitness(BigInt(i)));
      witnesses.push(serializeFields(witness.toFields()));
    }
    // Save witnesses to ../src/witnesses.ts
    const witnessesFileContent = `// Auto-generated witnesses for TABLE0 Merkle tree
// Generated from tree.test.ts

export const WITNESSES0: readonly string[] = [
${witnesses
  .map((witness, index) => `  "${witness}", // Index ${index}`)
  .join("\n")}
] as const;
`;

    // Write to file

    const witnessesPath = path.resolve("src/witnesses.ts");
    await fs.writeFile(witnessesPath, witnessesFileContent, "utf8");
    console.log(`✅ Saved ${witnesses.length} witnesses to ${witnessesPath}`);
  });
});
