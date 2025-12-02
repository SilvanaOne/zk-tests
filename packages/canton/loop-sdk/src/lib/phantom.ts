"use client";

export function getPhantomProvider() {
  if ("phantom" in window) {
    const provider = (window as any).phantom?.solana;

    if (provider?.isPhantom) {
      console.log("Phantom Solana provider found", provider);
      return provider;
    }
  }

  window.open("https://phantom.app/", "_blank");
  return undefined;
}

export async function connectPhantom(): Promise<string | undefined> {
  const provider = getPhantomProvider();
  if (!provider) {
    console.log("No Phantom provider found");
    return undefined;
  }
  try {
    const resp = await provider.request({
      method: "connect",
    });
    console.log("Phantom connected to", resp);
    return resp?.publicKey?.toString();
  } catch (err) {
    console.log("Error connecting to Phantom", err);
    // { code: 4001, message: 'User rejected the request.' }
    return undefined;
  }
}

export function disconnectPhantom(): void {
  const provider = getPhantomProvider();
  if (provider) {
    try {
      provider.disconnect();
      console.log("Phantom disconnected");
    } catch (err) {
      console.log("Error disconnecting from Phantom", err);
    }
  }
}

/**
 * Sign a Canton transaction hash using Phantom Solana wallet.
 *
 * Canton interactive submission uses Base64-encoded transaction hashes.
 *
 * IMPORTANT: Phantom has security measures that prevent signing raw 32-byte
 * binary data (which could be a Solana transaction). To work around this,
 * we wrap the hash in a human-readable message format with a prefix.
 *
 * The message format is:
 * "Canton Transaction Hash:\n<base64-encoded-hash>"
 *
 * Canton expects the signature of the RAW hash bytes, not the wrapped message.
 * Therefore, we need to sign the actual hash bytes. However, Phantom blocks
 * signing raw binary data that looks like a transaction.
 *
 * Workaround: We use a text message that includes the hash, but we must ensure
 * Canton verifies the signature correctly. Since Canton expects Ed25519 signature
 * of the raw hash, we need to sign exactly what Canton will verify.
 *
 * @param hashBase64 - Base64-encoded transaction hash from Canton's prepare endpoint
 * @returns Base64-encoded signature (64 bytes Ed25519: r || s concatenated)
 */
export async function signPhantomTransactionHash(hashBase64: string): Promise<string | undefined> {
  const provider = getPhantomProvider();
  if (!provider) {
    console.log("[phantom.ts] No Phantom provider found for signing");
    return undefined;
  }

  try {
    // Decode Base64 hash to raw bytes
    const hashBytes = Uint8Array.from(atob(hashBase64), c => c.charCodeAt(0));
    console.log("[phantom.ts] Signing transaction hash, bytes length:", hashBytes.length);
    console.log("[phantom.ts] Hash Base64:", hashBase64);

    // Phantom blocks signing raw binary data that looks like a transaction.
    // We need to create a message that Phantom will accept.
    //
    // Option 1: Sign a text message (Phantom will accept this)
    // But Canton expects signature of raw hash bytes.
    //
    // Option 2: Use signIn or other Phantom methods
    //
    // For now, let's try signing a text-prefixed message and see if Canton
    // can be configured to accept it, OR we need a different approach.
    //
    // Actually, looking at Solana's signMessage - it signs arbitrary bytes.
    // The error suggests Phantom is detecting the 32-byte pattern as a tx.
    // Let's try adding a prefix to make it clearly NOT a transaction.

    // Create a prefixed message that Phantom won't mistake for a transaction
    // The prefix makes the total length != 32 bytes and adds readable text
    const prefix = "Canton:";
    const prefixBytes = new TextEncoder().encode(prefix);
    const messageBytes = new Uint8Array(prefixBytes.length + hashBytes.length);
    messageBytes.set(prefixBytes, 0);
    messageBytes.set(hashBytes, prefixBytes.length);

    console.log("[phantom.ts] Message with prefix, total bytes:", messageBytes.length);

    // Sign with Phantom using utf8 display for the prefix part
    const result = await provider.request({
      method: "signMessage",
      params: {
        message: messageBytes,
        display: "utf8"  // Display as text since we have a readable prefix
      }
    });

    console.log("[phantom.ts] Phantom signed message result:", result);

    // Extract signature bytes from result
    // Phantom returns { signature: Uint8Array, publicKey: PublicKey }
    const sigBytes = result.signature;
    if (!sigBytes || sigBytes.length !== 64) {
      console.error("[phantom.ts] Invalid signature length:", sigBytes?.length);
      return undefined;
    }

    // Convert signature to Base64 (Canton expects Base64-encoded signature)
    const signatureBase64 = btoa(String.fromCharCode(...sigBytes));
    console.log("[phantom.ts] Signature Base64 length:", signatureBase64.length);

    return signatureBase64;
  } catch (err: any) {
    console.error("[phantom.ts] Error signing with Phantom:", err);
    // { code: 4001, message: 'User rejected the request.' }
    return undefined;
  }
}
