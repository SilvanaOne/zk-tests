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
}

export async function connectSolflare(): Promise<string | undefined> {
  const provider = getSolflareProvider();
  if (!provider) {
    console.log("No Solflare provider found");
    return;
  }
  try {
    const resp = await provider.connect();
    console.log("Solflare connection response:", resp);

    // The public key is available directly on the provider after connection
    const publicKey = provider.publicKey?.toString();
    console.log("Solflare connected successfully with address:", publicKey);
    return publicKey;
  } catch (err) {
    console.log("Error connecting to Solflare", err);
    // { code: 4001, message: 'User rejected the request.' }
  }
}

export async function signSolflareMessage(params: {
  message: string;
  display?: string;
}): Promise<{ signature: string; publicKey: string } | undefined> {
  const { message, display = "utf8" } = params;
  const provider = getSolflareProvider();
  if (!provider) {
    console.log("No Solflare provider found");
    return;
  }
  try {
    const encodedMessage = new TextEncoder().encode(message);
    console.log(
      "Encoded message",
      Array.from(encodedMessage)
        .map((b) => b.toString(16).padStart(2, "0"))
        .join("")
    );
    const signedMessage = await provider.signMessage(encodedMessage, display);
    console.log("Solflare signed message raw:", signedMessage);

    // Convert signature from Uint8Array to hex string
    const signature = Array.from(signedMessage.signature as Uint8Array)
      .map((b: number) => b.toString(16).padStart(2, "0"))
      .join("");

    const publicKey = signedMessage.publicKey?.toString();

    const result = { signature, publicKey };
    console.log("Solflare signed message processed:", result);
    return result;
  } catch (err) {
    console.log("Error signing message with Solflare", err);
    // { code: 4001, message: 'User rejected the request.' }
  }
}
