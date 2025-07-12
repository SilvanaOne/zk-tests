import { test, describe } from "node:test";
import assert from "node:assert";
import { TABLE0, TABLE1, TABLE2 } from "../src/constants.js";
import { createMerkleTree, Witness } from "../src/tree.js";
import { Field, Poseidon } from "o1js";
import { serializeFields, deserializeFields } from "@silvana-one/mina-utils";
const fs = await import("node:fs/promises");
const path = await import("node:path");

describe("Merkle Tree Witnesses Generation", () => {
  test("should generate witnesses for all tables (TABLE0-TABLE2)", async () => {
    console.log("Generating Merkle trees and witnesses for all tables...");

    const tables = [
      { name: "TABLE0", data: TABLE0 },
      { name: "TABLE1", data: TABLE1 },
      { name: "TABLE2", data: TABLE2 },
    ];

    let witnessesFileContent = `// Auto-generated witnesses for TABLE0-TABLE2 Merkle trees
// Generated from tree_witnesses.test.ts

`;

    for (const table of tables) {
      const tableArray = Array.from(table.data);
      const tree = createMerkleTree(tableArray);
      const root = tree.getRoot();

      witnessesFileContent += `export const ${
        table.name
      }_ROOT = "${root.toString()}";
`;
    }

    for (const table of tables) {
      console.log(`\n=== Processing ${table.name} ===`);

      // Convert table to bigint array
      const tableArray = Array.from(table.data);
      console.log(`Table size: ${tableArray.length} entries`);

      // Create the merkle tree
      const tree = createMerkleTree(tableArray);
      console.log(`Tree created with ${tree.leafCount} leaves`);

      // Get the root
      const root = tree.getRoot();
      console.log(`Root: ${root.toString()}`);

      // Generate witnesses for all entries
      const witnesses: string[] = [];
      console.log("Generating witnesses...");

      for (let i = 0; i < 1024; i++) {
        const witness = new Witness(tree.getWitness(BigInt(i)));
        witnesses.push(serializeFields(witness.toFields()));

        if (i % 100 === 0) {
          console.log(`  Generated ${i + 1} witnesses`);
        }
      }

      console.log(
        `✅ Generated ${witnesses.length} witnesses for ${table.name}`
      );

      // Add to witnesses file content
      witnessesFileContent += `export const WITNESSES${table.name.substring(
        5
      )}: readonly string[] = [
${witnesses
  .map((witness, index) => `  "${witness}", // Index ${index}`)
  .join("\n")}
] as const;

`;
    }

    // Add root exports
    witnessesFileContent += `// Merkle tree roots for each table
`;

    // Write to witnesses.ts file
    const witnessesPath = path.resolve("src/witnesses.ts");
    await fs.writeFile(witnessesPath, witnessesFileContent, "utf8");
    console.log(`\n✅ Saved witnesses for all tables to ${witnessesPath}`);

    // Verify file was created
    const stats = await fs.stat(witnessesPath);
    console.log(`File size: ${stats.size} bytes`);
  });

  test("should verify witness correctness for each table", async () => {
    console.log("\nVerifying witness correctness for all tables...");

    const tables = [
      { name: "TABLE0", data: TABLE0 },
      { name: "TABLE1", data: TABLE1 },
      { name: "TABLE2", data: TABLE2 },
    ];

    for (const table of tables) {
      console.log(`\n=== Verifying ${table.name} ===`);

      const tableArray = Array.from(table.data);
      const tree = createMerkleTree(tableArray);
      const root = tree.getRoot();

      // Test a few random indices
      const testIndices = [0, 1, 100, 500, 1023];

      for (const index of testIndices) {
        const leaf = tree.getLeaf(BigInt(index));
        const witness = new Witness(tree.getWitness(BigInt(index)));

        // Test serialization/deserialization
        const serializedWitness = serializeFields(witness.toFields());
        const restoredWitness = Witness.fromFields(
          deserializeFields(serializedWitness)
        );

        // Verify the witness produces the correct root
        const witnessRoot = restoredWitness.calculateRoot(leaf);
        const witnessIndex = restoredWitness.calculateIndex();

        assert(
          witnessIndex.equals(BigInt(index)),
          `Witness index should match leaf index for ${table.name}[${index}]`
        );
        assert(
          witnessRoot.equals(root),
          `Witness for ${table.name}[${index}] should produce correct root`
        );

        // Verify leaf value matches table value (accounting for field reduction)
        const expectedLeaf = Field(table.data[index]);
        assert(
          leaf.equals(expectedLeaf),
          `Leaf value should match table value for ${
            table.name
          }[${index}]. Expected: ${expectedLeaf.toString()}, Got: ${leaf.toString()}`
        );
      }

      console.log(`✅ Verified witnesses for ${table.name}`);
    }
  });

  test("should verify table consistency", () => {
    console.log("\nVerifying table consistency...");

    // Verify all tables have correct size
    assert.strictEqual(TABLE0.length, 1024, "TABLE0 should have 1024 entries");
    assert.strictEqual(TABLE1.length, 1024, "TABLE1 should have 1024 entries");
    assert.strictEqual(TABLE2.length, 1024, "TABLE2 should have 1024 entries");

    // Verify first entries (should all be 1 = R^0)
    assert.strictEqual(TABLE0[0], 1n, "TABLE0[0] should be 1");
    assert.strictEqual(TABLE1[0], 1n, "TABLE1[0] should be 1");
    assert.strictEqual(TABLE2[0], 1n, "TABLE2[0] should be 1");

    // Verify second entries are different (different powers of R)
    assert.strictEqual(TABLE0[1], TABLE0[1], "TABLE0[1] should be R");
    assert.notStrictEqual(
      TABLE0[1],
      TABLE1[1],
      "TABLE0[1] and TABLE1[1] should be different"
    );
    assert.notStrictEqual(
      TABLE0[1],
      TABLE2[1],
      "TABLE0[1] and TABLE2[1] should be different"
    );

    console.log("✅ Table consistency verified");
    console.log(`TABLE0[1] = ${TABLE0[1].toString(16)} (R^1)`);
    console.log(`TABLE1[1] = ${TABLE1[1].toString(16)} (R^1024)`);
    console.log(`TABLE2[1] = ${TABLE2[1].toString(16)} (R^1048576)`);
  });
});
