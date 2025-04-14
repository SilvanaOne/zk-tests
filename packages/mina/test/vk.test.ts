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
} from "o1js";

describe("Side loading", () => {
  it("should test side loading", async () => {
    const program1 = ZkProgram({
      name: "program1",
      publicInput: Field,
      methods: {
        check: {
          privateInputs: [],
          async method(publicInput: Field) {
            publicInput.assertEquals(Field(1));
          },
        },
      },
    });

    const program1bad = ZkProgram({
      name: "program1bad",
      publicInput: Field,
      methods: {
        check: {
          privateInputs: [],
          async method(publicInput: Field) {
            publicInput.assertEquals(Field(2));
          },
        },
      },
    });

    class NonRecursiveProof extends DynamicProof<Field, Void> {
      static publicInputType = Field;
      static publicOutputType = Void;
      static maxProofsVerified = 0 as const;
      static featureFlags = FeatureFlags.allMaybe;
    }

    const program2 = ZkProgram({
      name: "program2",
      publicInput: Field,
      methods: {
        check: {
          privateInputs: [NonRecursiveProof, VerificationKey, Field],
          async method(
            publicInput: Field,
            proof: NonRecursiveProof,
            vk: VerificationKey,
            vkHash: Field
          ) {
            vkHash.assertEquals(vk.hash);
            proof.verify(vk);
            proof.publicInput.assertEquals(publicInput);
          },
        },
      },
    });

    const cache: Cache = Cache.FileSystem("./cache");
    const program1Vk = (await program1.compile({ cache })).verificationKey;
    console.log("program1Vk", program1Vk.hash.toJSON());
    const program1badVk = (await program1bad.compile({ cache }))
      .verificationKey;
    console.log("program1badVk", program1badVk.hash.toJSON());
    const program2Vk = (await program2.compile({ cache })).verificationKey;
    console.log("program2Vk", program2Vk.hash.toJSON());
    const program1Proof = await program1.check(Field(1));
    const program1SideLoadedProof = NonRecursiveProof.fromProof(
      program1Proof.proof
    );
    const program1badProof = await program1bad.check(Field(2));
    const program1badSideLoadedProof = NonRecursiveProof.fromProof(
      program1badProof.proof
    );
    const program2Proof = await program2.check(
      Field(1),
      program1SideLoadedProof,
      program1Vk,
      program1Vk.hash
    );
    const ok1 = await verify(program2Proof.proof, program2Vk);
    console.log("ok1", ok1);
    program1badVk.hash = program1Vk.hash;
    const program2ProofBad = await program2.check(
      Field(2),
      program1badSideLoadedProof,
      program1badVk,
      program1Vk.hash
    );
    const ok2 = await verify(program2ProofBad.proof, program2Vk);
    console.log("ok2", ok2);
  });
});
