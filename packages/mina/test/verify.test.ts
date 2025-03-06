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

// const publicKey: Uint8Array = new Uint8Array([
//   3, 228, 135, 106, 12, 84, 227, 5, 206, 208, 117, 50, 124, 155, 224, 44, 1,
//   192, 183, 132, 137, 196, 154, 181, 104, 6, 17, 30, 47, 28, 64, 93, 236,
// ]);

// const publicKeyString: string =
//   "0x" +
//   Array.from(publicKey)
//     .map((b) => b.toString(16).padStart(2, "0"))
//     .join("")
//     .toUpperCase();
// const signature: Uint8Array = new Uint8Array([
//   99, 237, 151, 116, 234, 74, 25, 36, 164, 69, 181, 141, 222, 64, 141, 224, 166,
//   255, 245, 159, 184, 5, 193, 229, 206, 57, 228, 130, 12, 100, 137, 64, 103,
//   133, 69, 161, 25, 202, 201, 223, 102, 205, 84, 30, 162, 182, 172, 255, 24,
//   196, 178, 4, 196, 215, 87, 239, 157, 164, 224, 132, 168, 154, 80, 8,
// ]);

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
  it("should verify signature using secp256k1", async () => {
    const verified = secp256k1.ecdsaVerify(signature, hash, publicKey);

    console.log("verified", verified);
  });
  it("should create a signature using o1js", async () => {
    const privateKey = PrivateKey.random();
    const publicKey = privateKey.toPublicKey();
    const message: Field[] = Array.from(msg).map((b) => Field(b) as Field);
    const signature = Signature.create(privateKey, message);
    const verified = signature.verify(publicKey, message);

    console.log("verified - o1js", verified.toBoolean());
    const r = signature.r.toBigInt();
    const s = signature.s.toBigInt();
    console.log("r", r);
    console.log("s", s);
    const signature2 = Signature.fromJSON({ r: r.toString(), s: s.toString() });
    const verified2 = signature2.verify(publicKey, message);
    console.log("verified2 - o1js", verified2.toBoolean());
  });

  it("should verify signature using o1js", async () => {
    const Secp256k1 = createForeignCurve(Crypto.CurveParams.Secp256k1);
    class Ecdsa extends createEcdsa(Secp256k1) {}
    // // a private key is a random scalar of secp256k1
    // let privateKey = Secp256k1.Scalar.random();

    // let publicKey1 = Secp256k1.generator.scale(privateKey);
    // // console.log("publicKey1", publicKey1.toBigint());
    // console.log("publicKeyString", publicKeyString);
    // const publicKey2 = Secp256k1.fromEthers(publicKeyString);
    // // console.log("publicKey2", publicKey2.toBigint());

    // // sign a message - this is not a provable method!
    // let signature = Ecdsa.sign(msg, privateKey.toBigInt());
    // // console.log("signature", signature.toBigInt());

    // const verified = signature.verify(Bytes.from(msg), publicKey1);
    // console.log("verified", verified.toBoolean());

    // const hash = Hash.SHA2_256.hash(msg);
    // const sha256 = crypto.createHash("sha256");
    // sha256.update(msg);
    // const hash2 = sha256.digest();
    // console.log(
    //   "hash1",
    //   Array.from(hash.bytes)
    //     .map((b) => b.toBigInt().toString(16).padStart(2, "0"))
    //     .join("")
    // );
    // console.log(
    //   "hash2",
    //   Array.from(hash2)
    //     .map((b) => b.toString(16).padStart(2, "0"))
    //     .join("")
    // );
    // const signature3 = Ecdsa.signHash(hash, privateKey.toBigInt());
    // const verified3 = signature3.verifySignedHash(hash, publicKey1);
    // console.log("verified3", verified3.toBoolean());
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
