import { describe, it } from "node:test";
import assert from "node:assert";
import {
  Mina,
  PrivateKey,
  DynamicProof,
  VerificationKey,
  Void,
  ZkProgram,
  Field,
  SmartContract,
  method,
  AccountUpdate,
  state,
  State,
  Cache,
  FeatureFlags,
  verify,
  PublicKey,
  Bool,
  Provable,
  UInt8,
  UInt32,
} from "o1js";

function createProgram(count: number) {
  const program = ZkProgram({
    name: "program",
    publicOutput: UInt32,
    methods: {
      check: {
        privateInputs: [Provable.Array(UInt32, count), UInt32],
        async method(data: UInt32[], idx: UInt32) {
          let nonIdx = UInt32.from(0);
          for (let i = 0; i < count; i++) {
            const nonIdxTemp = nonIdx.add(
              Provable.if(data[i].equals(idx), UInt32.from(0), UInt32.from(1))
            );
            const nonIdxTemp2 = Provable.witness(UInt32, () => nonIdxTemp);
            nonIdxTemp2.assertEquals(nonIdxTemp);
            nonIdx = nonIdxTemp2;
          }
          return { publicOutput: nonIdx };
        },
      },
    },
  });

  return program;
}

describe("Arry", () => {
  it("should calculate number of constraints", async () => {
    const counts = [1, 10, 100, 1000];
    for (const count of counts) {
      const program = createProgram(count);
      const methods = await program.analyzeMethods();
      console.log(`${count}: ${(methods as any).check.rows}`);
    }
  });
});
