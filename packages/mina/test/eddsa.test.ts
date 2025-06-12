import { describe, it } from "node:test";
import assert from "node:assert";
import { ZkProgram, Bool, Bytes } from "o1js";
import {
  createEddsa,
  createForeignTwisted,
  TwistedCurves,
} from "../src/eddsa/index.js";

// Create a custom Edwards25519 curve class
class Edwards25519 extends createForeignTwisted(TwistedCurves.Edwards25519) {}
class Scalar extends Edwards25519.Scalar {}
class Eddsa extends createEddsa(Edwards25519) {}
class Bytes32 extends Bytes(32) {}

// Define a ZkProgram that verifies EdDSA signatures
const eddsa = ZkProgram({
  name: "eddsa",
  publicInput: Bytes32,
  publicOutput: Bool,

  methods: {
    verifyEddsa: {
      privateInputs: [Eddsa, Edwards25519],
      async method(
        message: Bytes32,
        signature: Eddsa,
        publicKey: Edwards25519
      ) {
        return {
          publicOutput: signature.verify(message, publicKey),
        };
      },
    },
  },
});

// Example: Generate a signature and verify it
async function run() {}

describe("EDDSA", () => {
  it("should verify signature", async () => {
    const methods = await eddsa.analyzeMethods();
    console.log("rows:", (methods as any).verifyEddsa.rows);
    // Generate a keypair
    let privateKey = Edwards25519.Scalar.random();
    let publicKey = Edwards25519.generator.scale(privateKey);

    // Sign a message
    let message = Bytes32.fromString("Hi");
    let signature = Eddsa.sign(message.toBytes(), privateKey.toBigInt());
    const result = signature.verify(message, publicKey);
    console.log("result:", result.toBoolean());

    // Compile the program
    //await eddsa.compile();

    // Verify the signature in zk
    let { publicOutput } = await eddsa.rawMethods.verifyEddsa(
      message,
      signature,
      publicKey
    );

    console.log("valid:", publicOutput.toBoolean());
    // Check the result
    publicOutput.assertTrue("signature verifies");
  });
});
