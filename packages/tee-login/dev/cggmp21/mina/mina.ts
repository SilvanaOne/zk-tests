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
} from "o1js";

import secp256k1 from "secp256k1";

// created with https://github.com/LFDT-Lockness/cggmp21
/*
cargo test signing::signing_sync::secp256k1::t3n5 -- --nocapture
Signers: [4, 3, 2]
Signature: Signature { r: NonZero(Scalar { curve: "secp256k1", value: "a9350585110bbb2eb67be843117dbb2ff5de5bc918cdf1285be8d457f21ff6ed" }), s: NonZero(Scalar { curve: "secp256k1", value: "684b8facb4d2ca2a1630686448d56bb223c8e409cc0ca7f4f61a52f04fe6e1de" }) }
Public key: NonZero(Point { curve: "secp256k1", value: "02224332c5a2aacc26bab4ce65e5de9c62f5cf9c9570af130a2bad57ed3cbc24ef" })
Message to sign: DataToSign(Scalar { curve: "secp256k1", value: "8d073871bb6698dc8e57cb148cbb2d2597a425f4a657086d36f5ec4fa84b238d" })
Original message to sign: [60, 175, 6, 163, 197, 199, 151, 156, 67, 19, 104, 91, 98, 103, 101, 218, 161, 148, 193, 253, 253, 196, 138, 59, 101, 161, 82, 175, 137, 205, 166, 72, 158, 219, 129, 69, 160, 214, 25, 104, 67, 109, 30, 203, 146, 226, 168, 156, 13, 238, 25, 186, 88, 82, 228, 59, 156, 175, 212, 148, 21, 183, 121, 197, 117, 120, 81, 10, 55, 38, 59, 136, 106, 100, 4, 194, 33, 146, 179, 81, 7, 225, 246, 120, 174, 136, 61, 174, 154, 188, 79, 253, 29, 223, 73, 60, 90, 159, 190, 118]
test signing::signing_sync::secp256k1::t3n5 ... ok

*/
const SIGNATURE =
  "a9350585110bbb2eb67be843117dbb2ff5de5bc918cdf1285be8d457f21ff6ed684b8facb4d2ca2a1630686448d56bb223c8e409cc0ca7f4f61a52f04fe6e1de";
const ADDRESS =
  "02224332c5a2aacc26bab4ce65e5de9c62f5cf9c9570af130a2bad57ed3cbc24ef";
const MESSAGE_HASHED =
  "8d073871bb6698dc8e57cb148cbb2d2597a425f4a657086d36f5ec4fa84b238d";
const original_message = Uint8Array.from([
  60, 175, 6, 163, 197, 199, 151, 156, 67, 19, 104, 91, 98, 103, 101, 218, 161,
  148, 193, 253, 253, 196, 138, 59, 101, 161, 82, 175, 137, 205, 166, 72, 158,
  219, 129, 69, 160, 214, 25, 104, 67, 109, 30, 203, 146, 226, 168, 156, 13,
  238, 25, 186, 88, 82, 228, 59, 156, 175, 212, 148, 21, 183, 121, 197, 117,
  120, 81, 10, 55, 38, 59, 136, 106, 100, 4, 194, 33, 146, 179, 81, 7, 225, 246,
  120, 174, 136, 61, 174, 154, 188, 79, 253, 29, 223, 73, 60, 90, 159, 190, 118,
]);

const signature = Uint8Array.from(
  SIGNATURE.match(/.{1,2}/g)?.map((byte) => parseInt(byte, 16)) ?? []
);

const publicKey = Uint8Array.from(
  ADDRESS.match(/.{1,2}/g)?.map((byte) => parseInt(byte, 16)) ?? []
);

const message = Uint8Array.from(
  MESSAGE_HASHED.match(/.{1,2}/g)?.map((byte) => parseInt(byte, 16)) ?? []
);

class Bytes100 extends Bytes(100) {}
class Secp256k1 extends createForeignCurve(Crypto.CurveParams.Secp256k1) {}
class Ecdsa extends createEcdsa(Secp256k1) {}

const zkProgram = ZkProgram({
  name: "cggmp21",
  publicInput: Bytes100,
  publicOutput: Bool,

  methods: {
    verifyEcdsa: {
      privateInputs: [Ecdsa, Secp256k1],
      async method(message: Bytes100, signature: Ecdsa, publicKey: Secp256k1) {
        const hash = Hash.SHA2_256.hash(message);
        const verified = signature.verifySignedHash(hash, publicKey);
        return {
          publicOutput: verified,
        };
      },
    },
  },
});

async function main() {
  console.log("Signature length", signature.length);
  console.log("original_message length", original_message.length);
  const isValid = secp256k1.ecdsaVerify(signature, message, publicKey);

  console.log("secp256k1 lib: signature valid:", isValid);

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

  const hashedMessageBytes: Bytes = Bytes.from(message);

  const signatureMina = Ecdsa.from({ r, s });
  const hash = Hash.SHA2_256.hash(Bytes.from(original_message));
  assert.deepEqual(hash, hashedMessageBytes);
  const publicKeyMina = Secp256k1.fromEthers("0x" + ADDRESS);
  const verifiedMina = signatureMina.verifySignedHash(hash, publicKeyMina);
  console.log("o1js lib: signature valid:", verifiedMina.toBoolean());
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
      Bytes.from(original_message),
      signatureMina,
      publicKeyMina
    )
  ).proof;
  console.log("proof", {
    publicInput: proof.publicInput.toBytes(),
    publicOutput: proof.publicOutput.toBoolean(),
  });
  console.timeEnd("prove");
  console.time("verify");
  const verifiedZkProgram = await verify(proof, vk);
  console.timeEnd("verify");
  console.log("verifiedZkProgram", verifiedZkProgram);
}

main().catch(console.error);
