import { describe, it } from "node:test";
import assert from "node:assert";
import { ZkProgram, Cache, verify, Field, Poseidon } from "o1js";

const hashProgram = ZkProgram({
  name: "hash",
  publicOutput: Field,

  methods: {
    hash: {
      privateInputs: [Field, Field, Field],
      async method(message1: Field, message2: Field, message3: Field) {
        const hash = Poseidon.hash([message1, message2, message3]);

        return {
          publicOutput: hash,
        };
      },
    },
  },
});

describe("Proof of Poseidon hash", async () => {
  it("should hash", async () => {
    const message = [Field(1n), Field(2n), Field(3n)];
    const hash = Poseidon.hash(message);
    console.log("hash:", hash.toJSON()); // 24619730558757750532171846435738270973938732743182802489305079455910969360336n
    console.log("hash hex:", "0x" + hash.toBigInt().toString(16)); // 0x366e46102b0976735ed1cc8820c7305822a448893fee8ceeb42a3012a4663fd0

    assert.strictEqual(
      hash.toBigInt(),
      24619730558757750532171846435738270973938732743182802489305079455910969360336n
    );

    const cache = Cache.FileSystem("./cache");
    const methods = await hashProgram.analyzeMethods();

    // Detailed circuit statistics similar to rollup.test.ts
    const hashMethodStats = (methods as any).hash;

    console.log(`\n=== HASH METHOD CIRCUIT STATISTICS ===`);
    console.log(`Rows (constraints): ${hashMethodStats.rows}`);
    console.log(`Digest: ${hashMethodStats.digest}`);
    console.log(`\nGates breakdown:`);
    console.log(`  - Total gates: ${hashMethodStats.gates.length}`);

    // Count gate types and their wire counts
    const gateTypes = new Map<string, { count: number; totalWires: number }>();
    for (const gate of hashMethodStats.gates) {
      const typ = gate?.type || gate?.typ || "Unknown";
      const wireCount = gate?.wires?.length || 0;

      if (!gateTypes.has(typ)) {
        gateTypes.set(typ, { count: 0, totalWires: 0 });
      }
      const stats = gateTypes.get(typ)!;
      stats.count++;
      stats.totalWires += wireCount;
    }

    console.log(`  - Gate types breakdown:`);
    for (const [type, stats] of gateTypes.entries()) {
      const avgWires = stats.totalWires / stats.count;
      console.log(
        `    * ${type}: ${stats.count} gates, ${
          stats.totalWires
        } total wires (avg: ${avgWires.toFixed(2)} wires/gate)`
      );
    }

    // Additional gate analysis
    console.log(`\nDetailed gate analysis:`);
    for (const [type, stats] of gateTypes.entries()) {
      const percentage = (
        (stats.count / hashMethodStats.gates.length) *
        100
      ).toFixed(2);
      console.log(`  - ${type}: ${stats.count} gates (${percentage}%)`);
    }

    // Wire complexity analysis
    let totalWires = 0;
    let maxWiresPerGate = 0;
    for (const gate of hashMethodStats.gates) {
      if (gate.wires) {
        const wireCount = gate.wires.length;
        totalWires += wireCount;
        maxWiresPerGate = Math.max(maxWiresPerGate, wireCount);
      }
    }
    const avgWiresPerGate = totalWires / hashMethodStats.gates.length;

    console.log(`\nWire complexity:`);
    console.log(`  - Total wires: ${totalWires}`);
    console.log(`  - Average wires per gate: ${avgWiresPerGate.toFixed(2)}`);
    console.log(`  - Max wires per gate: ${maxWiresPerGate}`);

    // Coefficient analysis for gates with coeffs
    let gatesWithCoeffs = 0;
    let totalCoeffs = 0;
    let maxCoeffsPerGate = 0;
    for (const gate of hashMethodStats.gates) {
      if (gate.coeffs && gate.coeffs.length > 0) {
        gatesWithCoeffs++;
        totalCoeffs += gate.coeffs.length;
        maxCoeffsPerGate = Math.max(maxCoeffsPerGate, gate.coeffs.length);
      }
    }

    console.log(`\nCoefficient analysis:`);
    console.log(`  - Gates with coefficients: ${gatesWithCoeffs}`);
    console.log(`  - Total coefficients: ${totalCoeffs}`);
    if (gatesWithCoeffs > 0) {
      console.log(
        `  - Average coefficients per gate with coeffs: ${(
          totalCoeffs / gatesWithCoeffs
        ).toFixed(2)}`
      );
      console.log(`  - Max coefficients per gate: ${maxCoeffsPerGate}`);
    }

    console.log(`\n${"=".repeat(50)}`);
    console.time("compile");
    const vk = (
      await hashProgram.compile({
        cache,
      })
    ).verificationKey;
    console.timeEnd("compile");

    console.time("prove");
    const proof = (await hashProgram.hash(message[0], message[1], message[2]))
      .proof;

    console.timeEnd("prove");
    console.log("proof", {
      publicOutput: proof.publicOutput.toJSON(),
    });
    console.time("verify");
    assert.strictEqual(proof.publicOutput.toBigInt(), hash.toBigInt());
    const verified = await verify(proof, vk);
    console.timeEnd("verify");
    console.log("verified", verified);
    assert.strictEqual(verified, true);
  });
});

/*
Output:

> npm run test test/hash.test.ts


> zk-tests-mina@0.1.0 test
> NODE_NO_WARNINGS=1 node -r ./log.cjs --loader=ts-node/esm --enable-source-maps -r dotenv/config --require dotenv/config --env-file=.env --test test/hash.test.ts

[14:10:48.892] hash: 24619730558757750532171846435738270973938732743182802489305079455910969360336
[14:10:48.893] hash hex: 0x366e46102b0976735ed1cc8820c7305822a448893fee8ceeb42a3012a4663fd0
[14:10:49.317] 
=== HASH METHOD CIRCUIT STATISTICS ===
[14:10:49.317] Rows (constraints): 25
[14:10:49.317] Digest: 7c7519e555de1bfb3bc9b3456e9a936e
[14:10:49.317] 
Gates breakdown:
[14:10:49.317]   - Total gates: 25
[14:10:49.317]   - Gate types breakdown:
[14:10:49.317]     * Poseidon: 22 gates, 154 total wires (avg: 7.00 wires/gate)
[14:10:49.317]     * Zero: 2 gates, 14 total wires (avg: 7.00 wires/gate)
[14:10:49.317]     * Generic: 1 gates, 7 total wires (avg: 7.00 wires/gate)
[14:10:49.317] 
Detailed gate analysis:
[14:10:49.317]   - Poseidon: 22 gates (88.00%)
[14:10:49.317]   - Zero: 2 gates (8.00%)
[14:10:49.317]   - Generic: 1 gates (4.00%)
[14:10:49.317] 
Wire complexity:
[14:10:49.317]   - Total wires: 175
[14:10:49.317]   - Average wires per gate: 7.00
[14:10:49.317]   - Max wires per gate: 7
[14:10:49.317] 
Coefficient analysis:
[14:10:49.317]   - Gates with coefficients: 23
[14:10:49.317]   - Total coefficients: 340
[14:10:49.317]   - Average coefficients per gate with coeffs: 14.78
[14:10:49.317]   - Max coefficients per gate: 15
[14:10:49.317] 
==================================================
[14:10:50.154] compile: 837.53ms
[14:10:59.261] prove: 9.106s
[14:10:59.261] proof {
  publicOutput: '24619730558757750532171846435738270973938732743182802489305079455910969360336'
}
[14:11:00.020] verify: 758.977ms
[14:11:00.020] verified true
*/
