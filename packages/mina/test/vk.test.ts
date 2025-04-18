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
    console.log("Verifying bad proof with bad vk");
    console.time("bad 1");
    program1badSideLoadedProof.verify(program1badVk);
    console.timeEnd("bad 1");
    console.log("Verifying bad proof with good vk");
    console.time("bad 2");
    program1badSideLoadedProof.verify(program1Vk);
    console.timeEnd("bad 2");
    console.log("Bad proofs verified");

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
  it.skip("should test side loading - other example", async () => {
    const { privateKey, publicKey } = PrivateKey.randomKeypair();

    const p1 = ZkProgram({
      name: "asd",
      publicInput: PublicKey,
      publicOutput: Bool,
      methods: {
        test: {
          privateInputs: [PrivateKey],
          async method(
            publicInput: PublicKey,
            privateInput: PrivateKey
          ): Promise<{ publicOutput: Bool }> {
            const b = privateInput.toPublicKey().equals(publicInput);
            return { publicOutput: b };
          },
        },
      },
    });

    const p1vk = await p1.compile();

    const p2 = ZkProgram({
      name: "bsd",
      publicInput: PublicKey,
      publicOutput: Bool,
      methods: {
        test: {
          privateInputs: [],
          async method(
            publicInput: PublicKey
          ): Promise<{ publicOutput: Bool }> {
            const b = Bool(true);
            return { publicOutput: b };
          },
        },
      },
    });

    const p2vk = await p2.compile();
    console.log("p1vk hash", p1vk.verificationKey.hash.toJSON());
    console.log("p2vk hash", p2vk.verificationKey.hash.toJSON());

    const proof = await p2.test(publicKey);

    const p3vk = p2vk;
    p3vk.verificationKey.hash = p1vk.verificationKey.hash;

    class DProof extends DynamicProof<PublicKey, Bool> {
      static publicSpecType = PublicKey;
      static publicOutputType = Bool;
      static maxProofsVerified = 0 as const;
      // we may want to consider from program list for potential optimizations
      // but this one is universal and dynamic
      static featureFlags = FeatureFlags.allMaybe;
    }

    const dproof = DProof.fromProof(proof.proof);

    dproof.verify(p3vk.verificationKey);
  });
});
