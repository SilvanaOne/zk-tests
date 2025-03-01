import {
  bitsToBytes,
  bytesToBits,
  record,
  withVersionNumber,
} from "./binable.js";
import { Field } from "./field-bigint.js";
import {
  Group,
  Scalar,
  PrivateKey,
  versionNumbers,
  PublicKey,
} from "./curve-bigint.js";
import { base58 } from "./base58.js";
import { versionBytes } from "./constants.js";
import type {
  SignedLegacy,
  SignatureJson,
  Signed,
  NetworkId,
  SignedRosetta,
} from "./types.js";
import { sign, Signature, verify } from "./signature.js";

/**
 * Verifies a signature created by {@link signFields}.
 *
 * @param signedFields The signed field elements
 * @returns True if the `signedFields` contains a valid signature matching
 * the fields and publicKey.
 */
export function verifyFields({ data, signature, publicKey }: Signed<bigint[]>) {
  /*
      verifyFields {
        signature: {
          r: 19996910013141570341263734673999978016031842709489071252992906391155381778902n,
          s: 3955505917773286787189402766131368989164806632077003535544705187409914383142n
        },
        publicKey: {
          x: 23870790172301888504759036806304867767472357997524493282691794869180801897430n,
          isOdd: true
        }
      }
  */
  return verify(
    Signature.fromBase58(signature),
    { fields: data },
    PublicKey.fromBase58(publicKey),
    "testnet"
  );
}
