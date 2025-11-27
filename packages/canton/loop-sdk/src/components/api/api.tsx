"use client";
import { useImperativeHandle, forwardRef } from "react";

// ---------- types the parent will see ----------
export interface ApiFrameHandle {
  /** returns hex-encoded signature */
  signMessage(msg: bigint[], publicKey: string): Promise<string>;
  signPayment(payment: string, publicKey: string): Promise<string | undefined>;
  privateKeyId(): Promise<{ privateKeyId: string; publicKey: string }>;
  decryptShares(data: string[], privateKeyId: string): Promise<string>;
  verifyAttestation(attestation: string): Promise<string>;
}

// -------------- Mock implementations ------------------
// These replace the iframe-based ui-api communication with mock data

function generateMockHexSignature(): string {
  return (
    "0x" +
    Array(128)
      .fill(0)
      .map(() => Math.floor(Math.random() * 16).toString(16))
      .join("")
  );
}

// -------------- the component ------------------
export const Api = forwardRef<ApiFrameHandle>(function Api(_props, ref) {
  // expose mock implementations via ref
  useImperativeHandle(
    ref,
    (): ApiFrameHandle => ({
      async signMessage(_msg, _publicKey) {
        // Return a mock hex-encoded signature
        return generateMockHexSignature();
      },

      async signPayment(payment, _publicKey) {
        // Return the payment with a mock signature appended
        const parsed = JSON.parse(payment);
        return JSON.stringify({
          data: parsed,
          signature: {
            field: "mock_field_" + Math.random().toString(36).substring(7),
            scalar: "mock_scalar_" + Math.random().toString(36).substring(7),
          },
        });
      },

      async privateKeyId() {
        // Return mock private key ID and public key
        return {
          privateKeyId: "mock-private-key-id-" + Date.now(),
          publicKey:
            "B62qmockpublickey" + Math.random().toString(36).substring(2, 15),
        };
      },

      async decryptShares(_data, _privateKeyId) {
        // Return mock decrypted mnemonic (standard BIP39 test mnemonic)
        return "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
      },

      async verifyAttestation(_attestation) {
        // Return mock verification result
        return JSON.stringify({ verified: true, mock: true });
      },
    }),
    []
  );

  // No iframe needed - return null
  return null;
});
