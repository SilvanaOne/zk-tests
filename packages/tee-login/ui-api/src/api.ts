// src/api.ts
// Compile with `tsc` or bundle with esbuild / Rollup → a single JS file.

import { initSync, verify_attestation } from "./pkg/precompiles.js";
import { signMessage, signPayment } from "./mina.js";
import { getWasmBytes } from "./embedded-wasm.js";

import { b64ToBytes, bytesToHex, secureZero, uuid } from "./utils";
import { SecretStore } from "./secrets.js";
import { generateKeyPair, decryptShares } from "./encrypt.js";

const secrets = new SecretStore();
const keys = new Map<string, CryptoKey>();

/* ---------- Debug logging helper ---------- */
declare const DEBUG: boolean;
const debug = (...args: any[]) => {
  if (typeof DEBUG !== "undefined" && DEBUG) {
    console.log("[iframe]", ...args);
  }
};

/* ---------- message contract ---------- */
type PrivateKeyIdRequest = { id: string; type: "private_key_id" };
type DecryptSharesRequest = {
  id: string;
  type: "decrypt_shares";
  data: string[];
  privateKeyId: string;
};
type SignMessageRequest = {
  id: string;
  type: "sign_message";
  msg: bigint[];
  publicKey: string;
};
type SignPaymentRequest = {
  id: string;
  type: "sign_payment";
  payment: string;
  publicKey: string;
};
type VerifyAttestationRequest = {
  id: string;
  type: "verify_attestation";
  attestation: string;
};

export type ApiRequest =
  | SignMessageRequest
  | SignPaymentRequest
  | PrivateKeyIdRequest
  | DecryptSharesRequest
  | VerifyAttestationRequest;

type PrivateKeyIdResponse = {
  id: string;
  type: "private_key_id";
  value: { privateKeyId: string; publicKey: string };
};
type DecryptSharesResponse = {
  id: string;
  type: "decrypt_shares";
  value: string;
};
type SignMessageResponse = { id: string; type: "sign_message"; value: string }; // hex
type SignPaymentResponse = {
  id: string;
  type: "sign_payment";
  value: string | undefined;
}; // hex
type VerifyAttestationResponse = {
  id: string;
  type: "verify_attestation";
  value: string;
};
type ErrorResponse = { id: string; type: "error"; reason: string };
type ReadyResponse = { type: "ready" };

export type ApiResponse =
  | PrivateKeyIdResponse
  | DecryptSharesResponse
  | SignMessageResponse
  | SignPaymentResponse
  | ErrorResponse
  | ReadyResponse
  | VerifyAttestationResponse;

(async () => {
  debug("starting up");

  // 1️⃣   Load & instantiate the WASM module
  initSync({ module: getWasmBytes() }); // Use embedded WASM bytes, no fetch needed

  // Signal that the iframe is ready
  debug("sending ready message");
  parent.postMessage({ type: "ready" }, "*");

  /* 2️⃣  Listen for requests */
  self.addEventListener("message", async (ev: MessageEvent<ApiRequest>) => {
    debug("received message", ev.data);

    // In sandboxed iframes without allow-same-origin, ev.source might be null
    // We rely on the fact that:
    // 1. Only our parent can send messages to this iframe
    // 2. We validate the message structure
    // 3. The iframe is sandboxed and can only run our trusted code

    // Basic message validation
    if (
      !ev.data ||
      typeof ev.data !== "object" ||
      !ev.data.id ||
      !ev.data.type
    ) {
      debug("invalid message format, ignoring");
      return;
    }

    const { id, type } = ev.data;
    try {
      switch (type) {
        case "private_key_id": {
          debug("processing private_key_id request", { id });
          const { privateKey, publicKey } = await generateKeyPair();
          // Start of Selection
          if (privateKey === null) {
            throw new Error("Failed to generate key pair");
          }
          const privateKeyId = uuid();
          keys.set(privateKeyId, privateKey);
          const resp: PrivateKeyIdResponse = {
            id,
            type: "private_key_id",
            value: { privateKeyId, publicKey },
          };
          parent.postMessage(resp, "*");
          break;
        }
        case "decrypt_shares": {
          debug("processing decrypt_shares request", { id });
          const key = keys.get(ev.data.privateKeyId);
          if (!key) {
            throw new Error("Private key not found");
          }
          const decrypted = await decryptShares({
            data: ev.data.data,
            privateKey: key,
          });
          if (!decrypted) {
            throw new Error("Failed to decrypt shares");
          }
          const { publicKey, privateKey } = decrypted;
          secrets.add(publicKey, new TextEncoder().encode(privateKey));
          const resp: DecryptSharesResponse = {
            id,
            type: "decrypt_shares",
            value: publicKey,
          };
          parent.postMessage(resp, "*");
          break;
        }
        case "sign_message": {
          debug("processing sign message request", {
            id,
            msgLength: ev.data.msg.length,
          });

          const msg = ev.data.msg;
          const publicKey = ev.data.publicKey;
          const signature = await secrets.withSecret(
            publicKey,
            async (keyBytes) => {
              const key = new TextDecoder().decode(keyBytes);
              return signMessage(msg, key);
            }
          );

          if (!signature) {
            throw new Error("Failed to sign message");
          }

          const resp: SignMessageResponse = {
            id,
            type: "sign_message",
            value: signature,
          };
          debug("sending sign message response", {
            id,
            sigLength: resp.value?.length,
          });
          parent.postMessage(resp, "*");
          break;
        }

        case "sign_payment": {
          debug("processing sign payment request", {
            id,
            paymentLength: ev.data.payment.length,
          });

          const payment = ev.data.payment;
          const publicKey = ev.data.publicKey;
          const signature = await secrets.withSecret(
            publicKey,
            async (keyBytes) => {
              const key = new TextDecoder().decode(keyBytes);
              return signPayment({ payment, privateKey: key });
            }
          );

          if (!signature) {
            throw new Error("Failed to sign message");
          }

          const resp: SignPaymentResponse = {
            id,
            type: "sign_payment",
            value: signature,
          };
          debug("sending sign payment response", {
            id,
            sigLength: resp.value?.length,
          });
          parent.postMessage(resp, "*");
          break;
        }

        case "verify_attestation": {
          debug("processing verify attestation request", { id });
          const attestation = ev.data.attestation;
          const verification_result = verify_attestation(attestation);
          const resp: VerifyAttestationResponse = {
            id,
            type: "verify_attestation",
            value: verification_result,
          };
          parent.postMessage(resp, "*");
        }

        default:
          debug("unknown message type", type);
      }
    } catch (err) {
      debug("error processing message", err);
      const e: ErrorResponse = {
        id,
        type: "error",
        reason: (err as Error).message,
      };
      parent.postMessage(e, "*");
    }
  });

  debug("message listener setup complete");
})();
