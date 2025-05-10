import { describe, it } from "node:test";
import assert from "node:assert";
import {
  createForeignCurve,
  Crypto,
  createEcdsa,
  Bytes,
  Hash,
  ZkProgram,
  Bool,
  Cache,
  verify,
  Signature,
  PrivateKey,
  PublicKey,
  Field,
} from "o1js";
import crypto from "node:crypto";
import secp256k1 from "secp256k1";

const messageHash =
  "039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81";
const signedData =
  "63ed9774ea4a1924a445b58dde408de0a6fff59fb805c1e5ce39e4820c648940678545a119cac9df66cd541ea2b6acff18c4b204c4d757ef9da4e084a89a5008";
const publicKeyString =
  "03e4876a0c54e305ced075327c9be02c01c0b78489c49ab56806111e2f1c405dec";
const signature = Uint8Array.from(
  signedData.match(/.{1,2}/g)?.map((byte) => parseInt(byte, 16)) ?? []
);
const hash = Uint8Array.from(
  messageHash.match(/.{1,2}/g)?.map((byte) => parseInt(byte, 16)) ?? []
);
const publicKey = Uint8Array.from(
  publicKeyString.match(/.{1,2}/g)?.map((byte) => parseInt(byte, 16)) ?? []
);

const msg: Uint8Array = new Uint8Array([1, 2, 3]);

class Bytes3 extends Bytes(3) {}
class Secp256k1 extends createForeignCurve(Crypto.CurveParams.Secp256k1) {}
class Scalar extends Secp256k1.Scalar {}
class Ecdsa extends createEcdsa(Secp256k1) {}

const zkProgram = ZkProgram({
  name: "ecdsa",
  publicInput: Bytes3,
  publicOutput: Bool,

  methods: {
    verifyEcdsa: {
      privateInputs: [Ecdsa, Secp256k1, Signature, PublicKey],
      async method(
        message: Bytes3,
        signature: Ecdsa,
        publicKey: Secp256k1,
        minaSignature: Signature,
        minaPublicKey: PublicKey
      ) {
        const hash = Hash.SHA2_256.hash(message);
        const verified1 = signature.verifySignedHash(hash, publicKey);
        const verified2 = minaSignature.verify(
          minaPublicKey,
          message.toFields()
        );
        const verified = verified1.and(verified2);
        return {
          publicOutput: verified,
        };
      },
    },
  },
});

describe("Check ECDSA signature", async () => {
  it("should create signature", async () => {
    // Generate a random private key
    const privateKey = secp256k1.
    const privateKeyBuffer = Uint8Array.from(privateKey);

    // Hash the message (required for signing)
    const messageHash = crypto.createHash("sha256").update(msg).digest();

    // Sign the message hash with the private key
    const signatureObj = secp256k1.ecdsaSign(messageHash, privateKeyBuffer);

    // Get the signature in the format we need
    const ecdsaSignature = new Uint8Array([
      ...signatureObj.signature,
      signatureObj.recid,
    ]);

    // Derive the public key from the private key
    const publicKeyBuffer = secp256k1.publicKeyCreate(privateKeyBuffer);

    // Verify the signature
    const isValid = secp256k1.ecdsaVerify(
      signatureObj.signature,
      messageHash,
      publicKeyBuffer
    );

    console.log("Message:", msg.toString());
    console.log("Private key:", Buffer.from(privateKeyBuffer).toString("hex"));
    console.log("Public key:", Buffer.from(publicKeyBuffer).toString("hex"));
    console.log("Signature:", Buffer.from(ecdsaSignature).toString("hex"));
    console.log("Signature valid:", isValid);

    const Secp256k1 = createForeignCurve(Crypto.CurveParams.Secp256k1);
    class Ecdsa extends createEcdsa(Secp256k1) {}

    const r = BigInt(
      "0x" +
        Array.from(ecdsaSignature.slice(0, 32))
          .map((b) => b.toString(16).padStart(2, "0"))
          .join("")
    );
    const s = BigInt(
      "0x" +
        Array.from(ecdsaSignature.slice(32, 64))
          .map((b) => b.toString(16).padStart(2, "0"))
          .join("")
    );

    const publicKey = Secp256k1.fromEthers(publicKeyBuffer.toString());
    const signature = Ecdsa.from({ r, s });
    const hash = Hash.SHA2_256.hash(msg);
    const verified = signature.verifySignedHash(hash, publicKey);
    console.log("verified", verified.toBoolean());
  });

  it.skip("should verify signature using o1js", async () => {
    const Secp256k1 = createForeignCurve(Crypto.CurveParams.Secp256k1);
    class Ecdsa extends createEcdsa(Secp256k1) {}

    const r = BigInt(
      "0x" +
        Array.from(signature.slice(0, 32))
          .map((b) => b.toString(16).padStart(2, "0"))
          .join("")
    );
    const s = BigInt(
      "0x" +
        Array.from(signature.slice(32, 64))
          .map((b) => b.toString(16).padStart(2, "0"))
          .join("")
    );
    const privateKey = PrivateKey.random();
    const minaPublicKey = privateKey.toPublicKey();
    const message: Field[] = Array.from(msg).map((b) => Field(b) as Field);
    const minaSignature = Signature.create(privateKey, message);

    const signature2 = Ecdsa.from({ r, s });
    const hash2 = Hash.SHA2_256.hash(msg);
    // console.log("signature2", signature2.toBigInt());
    const publicKey2 = Secp256k1.fromEthers(publicKeyString);
    const verified2 = signature2.verifySignedHash(hash2, publicKey2);
    console.log("verified2", verified2.toBoolean());
    const cache = Cache.FileSystem("./cache");
    const methods = await zkProgram.analyzeMethods();
    console.log("rows", methods.verifyEcdsa.rows);
    console.time("compile");
    const vk = (
      await zkProgram.compile({
        cache,
      })
    ).verificationKey;
    console.timeEnd("compile");

    console.time("prove");
    const proof = (
      await zkProgram.verifyEcdsa(
        Bytes3.from(msg),
        signature2,
        publicKey2,
        minaSignature,
        minaPublicKey
      )
    ).proof;
    console.log("proof", {
      publicInput: proof.publicInput.toBytes(),
      publicOutput: proof.publicOutput.toBoolean(),
    });
    console.timeEnd("prove");
    console.time("verify");
    const verified3 = await verify(proof, vk);
    console.timeEnd("verify");
    console.log("verified3", verified3);
  });
});
