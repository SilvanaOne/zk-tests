"use client";

export function getSolflareProvider() {
  if ("solflare" in window) {
    const provider = (window as any).solflare;

    if (provider?.isSolflare) {
      console.log("Solflare provider found", provider);
      return provider;
    }
  }

  window.open("https://solflare.com/", "_blank");
  return undefined;
}

export async function connectSolflare(): Promise<string | undefined> {
  const provider = getSolflareProvider();
  if (!provider) {
    console.log("No Solflare provider found");
    return undefined;
  }
  try {
    await provider.connect();
    const publicKey = provider.publicKey?.toString();
    console.log("Solflare connected successfully with address:", publicKey);
    return publicKey;
  } catch (err) {
    console.log("Error connecting to Solflare", err);
    // { code: 4001, message: 'User rejected the request.' }
    return undefined;
  }
}

export function disconnectSolflare(): void {
  const provider = getSolflareProvider();
  if (provider) {
    try {
      provider.disconnect();
      console.log("Solflare disconnected");
    } catch (err) {
      console.log("Error disconnecting from Solflare", err);
    }
  }
}

/**
 * Sign a Canton transaction hash using Solflare wallet.
 *
 * Canton interactive submission uses Base64-encoded transaction hashes.
 * This function:
 * 1. Decodes the Base64 hash to raw bytes
 * 2. Signs the raw bytes with Solflare's Ed25519 key
 * 3. Returns the signature as Base64
 *
 * Unlike Phantom, Solflare does not have security restrictions that prevent
 * signing raw 32-byte binary data.
 *
 * @param hashBase64 - Base64-encoded transaction hash from Canton's prepare endpoint
 * @returns Base64-encoded signature (64 bytes Ed25519: r || s concatenated)
 */
export async function signSolflareTransactionHash(hashBase64: string): Promise<string | undefined> {
  const provider = getSolflareProvider();
  if (!provider) {
    console.log("[solflare.ts] No Solflare provider found for signing");
    return undefined;
  }

  try {
    // Decode Base64 hash to raw bytes
    const hashBytes = Uint8Array.from(atob(hashBase64), c => c.charCodeAt(0));
    console.log("[solflare.ts] Signing transaction hash, bytes length:", hashBytes.length);
    console.log("[solflare.ts] Hash Base64:", hashBase64);

    // Sign with Solflare - it signs raw bytes using Ed25519
    // display parameter is for UI display purposes
    const result = await provider.signMessage(hashBytes, "hex");

    console.log("[solflare.ts] Solflare signed message result:", result);

    // Extract signature bytes from result
    // Solflare returns { signature: Uint8Array, publicKey: PublicKey }
    const sigBytes = result.signature;
    if (!sigBytes || sigBytes.length !== 64) {
      console.error("[solflare.ts] Invalid signature length:", sigBytes?.length);
      return undefined;
    }

    // Convert signature to Base64 (Canton expects Base64-encoded signature)
    const signatureBase64 = btoa(String.fromCharCode(...sigBytes));
    console.log("[solflare.ts] Signature Base64 length:", signatureBase64.length);

    return signatureBase64;
  } catch (err: any) {
    console.error("[solflare.ts] Error signing with Solflare:", err);
    // { code: 4001, message: 'User rejected the request.' }
    return undefined;
  }
}
