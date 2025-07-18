import { describe, it } from "node:test";
import assert from "node:assert";
import {
  DynamicProof,
  VerificationKey,
  ZkProgram,
  Field,
  Cache,
  FeatureFlags,
  verify,
} from "o1js";

const program1 = ZkProgram({
  name: "program1",
  publicInput: Field,
  publicOutput: Field,
  methods: {
    add: {
      privateInputs: [],
      async method(publicInput: Field) {
        return { publicOutput: publicInput.add(1) };
      },
    },
  },
});
const program2 = ZkProgram({
  name: "program1",
  publicInput: Field,
  publicOutput: Field,
  methods: {
    add: {
      privateInputs: [],
      async method(publicInput: Field) {
        return { publicOutput: publicInput.add(2) };
      },
    },
  },
});

class NonRecursiveProof extends DynamicProof<Field, Field> {
  static publicInputType = Field;
  static publicOutputType = Field;
  static maxProofsVerified = 0 as const;
  static featureFlags = FeatureFlags.allMaybe;
}

const mergeProgram = ZkProgram({
  name: "mergeProgram",
  publicInput: Field,
  publicOutput: Field,
  methods: {
    merge: {
      privateInputs: [
        NonRecursiveProof,
        NonRecursiveProof,
        VerificationKey,
        VerificationKey,
      ],
      async method(
        publicInput: Field,
        proof1: NonRecursiveProof,
        proof2: NonRecursiveProof,
        vk1: VerificationKey,
        vk2: VerificationKey
      ) {
        proof1.verify(vk1);
        proof2.verify(vk2);
        proof1.publicInput.assertEquals(publicInput);
        proof2.publicInput.assertEquals(proof1.publicOutput);
        return { publicOutput: proof2.publicOutput };
      },
    },
  },
});

let vk1: VerificationKey | null = null;
let vk2: VerificationKey | null = null;
let mergeVk: VerificationKey | null = null;

describe("Merging two DynamicProofs", () => {
  it("should compile the programs", async () => {
    const cache: Cache = Cache.FileSystem("./cache");
    vk1 = (await program1.compile({ cache })).verificationKey;
    vk2 = (await program2.compile({ cache })).verificationKey;
    mergeVk = (await mergeProgram.compile({ cache })).verificationKey;
  });

  it("should test merging two DynamicProofs with the same program", async () => {
    assert(vk1 && vk2 && mergeVk, "Verification keys not found");
    const initialInput = Field(1);
    const proof1 = await program1.add(initialInput);
    const proof2 = await program1.add(proof1.proof.publicOutput);
    const dynamicProof1 = NonRecursiveProof.fromProof(proof1.proof);
    const dynamicProof2 = NonRecursiveProof.fromProof(proof2.proof);
    const mergeProof = await mergeProgram.merge(
      initialInput,
      dynamicProof1,
      dynamicProof2,
      vk1,
      vk1
    );
    assert.ok(
      mergeProof.proof.publicOutput
        .equals(proof2.proof.publicOutput)
        .toBoolean(),
      "Merging proofs failed"
    );
    const ok = await verify(mergeProof.proof, mergeVk);
    assert.ok(ok, "Merge proof verification failed");
  });
  it("should test merging two DynamicProofs with different programs", async () => {
    assert(vk1 && vk2 && mergeVk, "Verification keys not found");
    const initialInput = Field(1);
    const proof1 = await program1.add(initialInput);
    const proof2 = await program2.add(proof1.proof.publicOutput);
    const dynamicProof1 = NonRecursiveProof.fromProof(proof1.proof);
    const dynamicProof2 = NonRecursiveProof.fromProof(proof2.proof);
    const mergeProof = await mergeProgram.merge(
      initialInput,
      dynamicProof1,
      dynamicProof2,
      vk1,
      vk2
    );
    assert.ok(
      mergeProof.proof.publicOutput
        .equals(proof2.proof.publicOutput)
        .toBoolean(),
      "Merging proofs failed"
    );
    const ok = await verify(mergeProof.proof, mergeVk);
    assert.ok(ok, "Merge proof verification failed");
  });
});
