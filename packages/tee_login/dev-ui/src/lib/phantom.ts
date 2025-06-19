"use client";

export function getPhantomProvider(chain: string) {
  if ("phantom" in window) {
    const provider = (window.phantom as any)[chain];

    if (provider?.isPhantom) {
      console.log("Phantom provider found for", chain, provider);
      return provider;
    }
  }

  window.open("https://phantom.app/", "_blank");
}

export async function connectPhantom(
  chain: string
): Promise<string | undefined> {
  const provider = getPhantomProvider(chain); // see "Detecting the Provider"
  if (!provider) {
    console.log("No Phantom provider found for", chain);
    return;
  }
  try {
    if (chain === "sui") {
      const resp = await provider.requestAccount();
      console.log("Phantom account", resp);
      console.log(
        "Phantom connected to",
        resp,
        "with address",
        resp?.publicKey?.toString()
      );
      // 26qv4GCcx98RihuK3c4T6ozB3J7L6VwCuFVc7Ta2A3Uo
      return resp?.address;
    }
    if (chain === "solana") {
      const resp = await provider.request({
        method: "connect",
      });
      console.log("Phantom connected to", resp);
      return resp?.publicKey?.toString();
    }
    if (chain === "ethereum") {
      const accounts = await provider.request({
        method: "eth_requestAccounts",
      });
      console.log("Phantom connected to", accounts);
      return accounts[0];
    }
  } catch (err) {
    console.log("Error connecting to Phantom", err);
    // { code: 4001, message: 'User rejected the request.' }
  }
}

export async function signPhantomMessage(params: {
  chain: string;
  message: string;
  display: string;
}): Promise<string | undefined> {
  const { chain, message, display } = params;
  const provider = getPhantomProvider(params.chain); // see "Detecting the Provider"
  if (!provider) {
    console.log("No Phantom provider found for", chain);
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
    if (chain === "solana") {
      const signedMessage = await provider.request({
        method: "signMessage",
        params: {
          message: encodedMessage,
          display: "utf8",
        },
      });
      console.log("Phantom signed message", signedMessage);
      return signedMessage;
    }
    if (chain === "sui") {
      const resp = await provider.requestAccount();
      const address = resp.address;
      console.log("Sui Address", address);
      const signedMessage = await provider.signMessage(encodedMessage, address);
      console.log("Phantom signed message for Sui", signedMessage);
      return signedMessage;
    }
    if (chain === "ethereum") {
      const accounts = await provider.request({
        method: "eth_requestAccounts",
      });
      const from = accounts?.[0];
      console.log("Phantom connected to", from);
      if (!from) {
        console.log("No Ethereum account found");
        return;
      }
      const msg = `0x${Buffer.from(message, "utf8").toString("hex")}`;
      const signedMessage = await provider.request({
        method: "personal_sign",
        params: [msg, from, "Silvana TEE login"],
      });
      console.log("Phantom signed message", signedMessage);
      return signedMessage;
    }
  } catch (err) {
    console.log("Error connecting to Phantom", err);
    // { code: 4001, message: 'User rejected the request.' }
  }
}
