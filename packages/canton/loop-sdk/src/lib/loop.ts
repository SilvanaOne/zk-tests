"use client";
import { loop, LoopProvider, type Holding, type ActiveContract } from "@fivenorth/loop-sdk";

export type { ActiveContract };

export type LoopNetwork = "devnet" | "testnet" | "mainnet";

// Use window object to store provider to avoid module instance duplication issues in Next.js
declare global {
  interface Window {
    __loopProvider?: LoopProvider | null;
    __loopNetwork?: LoopNetwork;
    __loopCallbacks?: {
      onConnect?: (provider: LoopProvider) => void;
      onReject?: () => void;
    };
  }
}

function getProvider(): LoopProvider | null {
  if (typeof window === "undefined") return null;
  return window.__loopProvider ?? null;
}

function setProvider(provider: LoopProvider | null) {
  if (typeof window !== "undefined") {
    window.__loopProvider = provider;
  }
}

function getNetwork(): LoopNetwork {
  if (typeof window === "undefined") return "devnet";
  return window.__loopNetwork ?? "devnet";
}

function setNetwork(network: LoopNetwork) {
  if (typeof window !== "undefined") {
    window.__loopNetwork = network;
  }
}

function getCallbacks() {
  if (typeof window === "undefined") return {};
  return window.__loopCallbacks ?? {};
}

function setCallbacks(callbacks: { onConnect?: (provider: LoopProvider) => void; onReject?: () => void }) {
  if (typeof window !== "undefined") {
    window.__loopCallbacks = callbacks;
  }
}

export function initLoop(
  callbacks: { onConnect?: (provider: LoopProvider) => void; onReject?: () => void },
  network: LoopNetwork = "devnet"
) {
  setCallbacks(callbacks);
  setNetwork(network);

  loop.init({
    appName: "Silvana Wallet Connect",
    network: network,
    onAccept: (provider) => {
      console.log("[loop.ts] onAccept called with provider:", provider);
      setProvider(provider);
      getCallbacks().onConnect?.(provider);
    },
    onReject: () => {
      getCallbacks().onReject?.();
    },
  });
}

export function connectLoop(network?: LoopNetwork) {
  const currentNetwork = getNetwork();
  if (network && network !== currentNetwork) {
    // Clear old ticket when switching networks to force new ticket generation
    if (typeof window !== "undefined") {
      localStorage.removeItem("loop_connect");
    }
    setNetwork(network);
    loop.init({
      appName: "Silvana Wallet Connect",
      network: network,
      onAccept: (provider: LoopProvider) => {
        console.log("[loop.ts] connectLoop onAccept called with provider:", provider);
        setProvider(provider);
        getCallbacks().onConnect?.(provider);
      },
      onReject: () => {
        getCallbacks().onReject?.();
      },
    });
  }
  loop.connect();
}

export function getLoopProvider(): LoopProvider | null {
  return getProvider();
}

export function getCurrentNetwork(): LoopNetwork {
  return getNetwork();
}

export function disconnectLoop() {
  // Clear local storage to logout (Loop SDK stores session in 'loop_connect')
  if (typeof window !== "undefined") {
    localStorage.removeItem("loop_connect");
  }
  setProvider(null);
}

export async function getLoopHoldings(): Promise<Holding[] | null> {
  const provider = getProvider();
  console.log("[loop.ts] getLoopHoldings called, loopProvider:", provider);
  if (!provider) {
    console.log("[loop.ts] loopProvider is null, cannot fetch holdings");
    return null;
  }
  return provider.getHolding();
}

export async function getLoopActiveContracts(
  params?: { templateId?: string; interfaceId?: string }
): Promise<ActiveContract[] | null> {
  const provider = getProvider();
  console.log("[loop.ts] getLoopActiveContracts called, loopProvider:", provider);
  if (!provider) {
    console.log("[loop.ts] loopProvider is null, cannot fetch active contracts");
    return null;
  }
  return provider.getActiveContracts(params);
}

export async function signLoopMessage(message: string): Promise<any> {
  const provider = getProvider();
  console.log("[loop.ts] signLoopMessage called, message:", message);
  if (!provider) {
    console.log("[loop.ts] loopProvider is null, cannot sign message");
    return null;
  }
  return provider.signMessage(message);
}

export function getLoopPublicKey(): string | null {
  return getProvider()?.public_key ?? null;
}

/**
 * Verifies that a partyId's namespace is cryptographically derived from the public key.
 *
 * Canton Party ID format: identifier::1220fingerprint
 * Where fingerprint = SHA256(0x0000000C || public_key_bytes)
 * The 1220 prefix is multihash encoding (0x12=SHA-256, 0x20=32 bytes)
 */
export async function verifyPartyIdMatchesPublicKey(
  partyId: string,
  publicKeyHex: string
): Promise<boolean> {
  try {
    // Parse partyId: identifier::1220fingerprint
    const parts = partyId.split("::");
    if (parts.length !== 2) {
      console.log("[loop.ts] Invalid partyId format - expected identifier::namespace");
      return false;
    }

    const namespace = parts[1];

    // Namespace should start with 1220 (multihash prefix for SHA-256)
    if (!namespace.startsWith("1220")) {
      console.log("[loop.ts] Invalid namespace format - expected 1220 prefix");
      return false;
    }

    const expectedFingerprint = namespace.slice(4); // Remove 1220 prefix

    // Compute fingerprint: SHA256(purpose_id_4bytes || public_key_bytes)
    // HashPurpose.PublicKeyFingerprint = 12 (decimal) = 0x0000000C (4 bytes big-endian)
    const purposeBuffer = new ArrayBuffer(4);
    const purposeBytes = new Uint8Array(purposeBuffer);
    purposeBytes.set([0x00, 0x00, 0x00, 0x0c]);
    const publicKeyBytes = hexToBytes(publicKeyHex);

    // Concatenate purpose + public key
    const inputBuffer = new ArrayBuffer(purposeBytes.length + publicKeyBytes.length);
    const input = new Uint8Array(inputBuffer);
    input.set(purposeBytes, 0);
    input.set(publicKeyBytes, purposeBytes.length);

    // Compute SHA-256
    const hashBuffer = await crypto.subtle.digest("SHA-256", input);
    const hashArray = new Uint8Array(hashBuffer);
    const computedFingerprint = Array.from(hashArray)
      .map(b => b.toString(16).padStart(2, "0"))
      .join("");

    const isValid = computedFingerprint === expectedFingerprint;
    console.log("[loop.ts] PartyId verification:", {
      expected: expectedFingerprint,
      computed: computedFingerprint,
      isValid
    });

    return isValid;
  } catch (error) {
    console.error("[loop.ts] PartyId verification error:", error);
    return false;
  }
}

// Helper function to convert hex string to bytes
function hexToBytes(hex: string): Uint8Array<ArrayBuffer> {
  const buffer = new ArrayBuffer(hex.length / 2);
  const bytes = new Uint8Array(buffer);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.slice(i, i + 2), 16);
  }
  return bytes;
}

export async function verifyLoopSignature(
  message: string,
  signatureHex: string
): Promise<boolean> {
  const publicKeyHex = getLoopPublicKey();
  if (!publicKeyHex) {
    console.log("[loop.ts] No public key available for verification");
    return false;
  }

  // Parse signature if it's JSON (e.g., {"signature":"..."})
  let sigHex = signatureHex;
  try {
    const parsed = JSON.parse(signatureHex);
    if (parsed.signature) {
      sigHex = parsed.signature;
    }
  } catch {
    // Not JSON, use as-is
  }

  try {
    const publicKey = await crypto.subtle.importKey(
      "raw",
      hexToBytes(publicKeyHex),
      { name: "Ed25519" },
      false,
      ["verify"]
    );

    const isValid = await crypto.subtle.verify(
      "Ed25519",
      publicKey,
      hexToBytes(sigHex),
      new TextEncoder().encode(message)
    );

    console.log("[loop.ts] Signature verification result:", isValid);
    return isValid;
  } catch (error) {
    console.error("[loop.ts] Signature verification error:", error);
    return false;
  }
}
