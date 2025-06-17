"use client";

type EIP1193 = {
  isMetaMask?: boolean;
  isBraveWallet?: boolean;
  isPhantom?: boolean;
  request: (args: { method: string; params?: unknown[] }) => Promise<unknown>;
};

export function getMetaMaskProvider(): EIP1193 | null {
  const { ethereum } = window as any;
  if (!ethereum) return null;

  // If multiple providers are present, choose the real MetaMask
  if (Array.isArray(ethereum.providers)) {
    return (
      ethereum.providers.find(
        (p: EIP1193) => p.isMetaMask && !p.isPhantom && !p.isBraveWallet
      ) ?? null
    );
  }

  // Single provider case
  return ethereum.isMetaMask && !ethereum.isPhantom && !ethereum.isBraveWallet
    ? (ethereum as EIP1193)
    : null;
}

export async function connectMetaMask(): Promise<string | undefined> {
  const provider = await getMetaMaskProvider();
  if (!provider) {
    console.log("No MetaMask provider found");
    return;
  }
  try {
    try {
      const [address] = (await provider.request({
        method: "eth_requestAccounts",
      })) as string[];
      return address;
    } catch (err) {
      console.error("MetaMask connect error", err);
    }
  } catch (err) {
    console.log("Error connecting to MetaMask", err);
  }
}

export async function signMetaMaskMessage(params: {
  message: string;
  display?: string;
}): Promise<string | undefined> {
  const { message, display = "utf8" } = params;
  const provider = await getMetaMaskProvider();
  if (!provider) {
    console.log("No MetaMask provider found");
    return;
  }
  try {
    const [address] = (await provider.request({
      method: "eth_requestAccounts",
    })) as string[];
    console.log("MetaMask connected to", address);
    if (!address) {
      console.log("No Ethereum account found");
      return;
    }
    const msg = `0x${Buffer.from(message, "utf8").toString("hex")}`;
    const signedMessage = await provider.request({
      method: "personal_sign",
      params: [msg, address],
    });
    console.log("MetaMask signed message", signedMessage);
    return signedMessage as string;
  } catch (err) {
    console.log("Error signing message with MetaMask", err);
    // { code: 4001, message: 'User rejected the request.' }
  }
}
