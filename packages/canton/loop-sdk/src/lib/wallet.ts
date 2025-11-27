"use client";
import { connectMetaMask, signMetaMaskMessage } from "@/lib/metamask";
import { connectPhantom, signPhantomMessage } from "@/lib/phantom";
import { getWallets } from "@mysten/wallet-standard";
import { connectSolflare, signSolflareMessage } from "@/lib/solflare";
import { getMessage, LoginRequest, LoginResponse } from "./login";
import { WalletChain, WalletProvider, WalletType } from "./types";

export interface WalletOptionBase {
  id: string;
  name: string;
  logo: string;
  type: WalletType;
  description: string;
}
export interface WalletOptionWallet extends WalletOptionBase {
  type: "wallet";
  chain: WalletChain;
}

export interface WalletOptionSocial extends WalletOptionBase {
  type: "social";
  provider: WalletProvider; // For social logins
}

export type WalletOption = WalletOptionWallet | WalletOptionSocial;

export const walletOptions: WalletOption[] = [
  // Ethereum wallets
  {
    id: "metamask-ethereum",
    name: "MetaMask",
    chain: "ethereum",
    logo: "https://upload.wikimedia.org/wikipedia/commons/3/36/MetaMask_Fox.svg",
    type: "wallet",
    description: "Ethereum",
  },
  {
    id: "phantom-ethereum",
    name: "Phantom",
    chain: "ethereum",
    logo: "/phantom.svg",
    type: "wallet",
    description: "Ethereum",
  },

  // Sui wallets
  {
    id: "phantom-sui",
    name: "Phantom",
    chain: "sui",
    logo: "/phantom.svg",
    type: "wallet",
    description: "Sui",
  },
  {
    id: "slush-sui",
    name: "Slush",
    chain: "sui",
    logo: "/slush.svg",
    type: "wallet",
    description: "Sui",
  },

  // Solana wallets
  {
    id: "phantom-solana",
    name: "Phantom",
    chain: "solana",
    logo: "/phantom.svg",
    type: "wallet",
    description: "Solana",
  },
  {
    id: "solflare-solana",
    name: "Solflare",
    chain: "solana",
    logo: "/solflare.svg",
    type: "wallet",
    description: "Solana",
  },

  // Social logins
  {
    id: "google",
    name: "Google",
    logo: "https://cdn.jsdelivr.net/npm/simple-icons@v9/icons/google.svg",
    type: "social",
    description: "Google Login",
    provider: "google",
  },
  {
    id: "github",
    name: "GitHub",
    logo: "https://cdn.jsdelivr.net/npm/simple-icons@v9/icons/github.svg",
    type: "social",
    description: "GitHub Login",
    provider: "github",
  },
];

export interface WalletButtonProps {
  wallet: WalletOption;
  connected?: boolean;
  loading?: boolean;
  failed?: boolean;
  onClick: () => void;
}

// Helper functions to filter wallets by type
export const getWalletsByChain = (chain: WalletChain) =>
  walletOptions.filter(
    (wallet) => wallet.type === "wallet" && wallet.chain === chain
  );

export const getWalletsByProvider = (provider: WalletProvider) =>
  walletOptions.filter(
    (wallet) => wallet.type === "social" && wallet.provider === provider
  );

export const getWalletsByType = (type: WalletType) =>
  walletOptions.filter((wallet) => wallet.type === type);

export const getWalletById = (id: string) =>
  walletOptions.find((wallet) => wallet.id === id);

export async function connectWallet(
  walletId: string
): Promise<string | undefined> {
  const wallet = getWalletById(walletId);
  if (!wallet) {
    throw new Error(`Wallet with id ${walletId} not found`);
  }
  if (wallet.type === "social") {
    return undefined;
  }
  switch (walletId) {
    case "metamask-ethereum":
      return await connectMetaMask();
    case "phantom-ethereum":
      return await connectPhantom("ethereum");
    case "phantom-sui":
      return connectPhantom("sui");
    case "slush-sui":
      const availableWallets = getWallets().get();
      const wallet = availableWallets.find(
        (wallet) => wallet.name === "Slush"
      ) as any;
      const connected = await wallet?.features["standard:connect"].connect(); // connect call
      const address = connected?.accounts[0]?.address;
      return address as string | undefined;
    case "phantom-solana":
      return connectPhantom("solana");
    case "solflare-solana":
      return connectSolflare();
    default:
      throw new Error(`Unsupported wallet id: ${walletId}`);
  }
}

export async function signWalletMessage(params: {
  walletId: string;
  address: string;
  publicKey: string;
}): Promise<LoginRequest | undefined> {
  const { walletId, address, publicKey } = params;
  console.log("signWalletMessage called with", params);
  if (!address || !publicKey) {
    console.error("Address or public key not found");
    return undefined;
  }
  try {
    const wallet = getWalletById(walletId);
    console.log("wallet", wallet);
    if (!wallet) {
      console.error(`Wallet with id ${walletId} not found`);
      return undefined;
    }
    if (wallet.type === "social") {
      return undefined;
    }
    const msgData = await getMessage({
      login_type: "wallet",
      chain: wallet.chain,
      wallet: wallet.name,
      address,
      publicKey,
    });
    if (!msgData) {
      console.error("Failed to get message data");
      return undefined;
    }
    switch (walletId) {
      case "metamask-ethereum": {
        const signedMessage = await signMetaMaskMessage({
          message: msgData.request.message,
          display: "utf8",
        });
        if (!signedMessage) {
          console.error("User rejected message");
          return undefined;
        }
        const signature = signedMessage;
        if (!signature) {
          console.error("User rejected message");
          return undefined;
        }
        const request: LoginRequest = {
          ...msgData.request,
          signature,
        };
        return request;
      }
      case "phantom-ethereum": {
        const signedMessage = await signPhantomMessage({
          chain: "ethereum",
          message: msgData.request.message,
          display: "utf8",
        });
        if (!signedMessage) {
          console.error("User rejected message");
          return undefined;
        }
        const signature = signedMessage;
        if (!signature) {
          console.error("User rejected message");
          return undefined;
        }
        const request: LoginRequest = {
          ...msgData.request,
          signature,
        };
        return request;
      }
      case "phantom-sui": {
        const signedMessage = await signPhantomMessage({
          chain: "sui",
          message: msgData.request.message,
          display: "utf8",
        });
        const publicKey = (signedMessage as any)?.publicKey?.toString();
        console.log("Sui Public key", publicKey);
        const signature = (signedMessage as any)?.signature?.toString("hex");
        if (!signature) {
          console.error("User rejected message");
          return undefined;
        }
        const request: LoginRequest = {
          ...msgData.request,
          signature,
        };
        return request;
      }
      case "slush-sui": {
        const availableWallets = getWallets().get();
        const wallet = availableWallets.find(
          (wallet) => wallet.name === "Slush"
        ) as any;
        const connected = await wallet?.features["standard:connect"].connect(); // connect call
        const connectedAddress = connected?.accounts[0]?.address;
        if (!connectedAddress) {
          console.error("Address not found");
          return undefined;
        }
        if (connectedAddress !== address) {
          console.error("Address mismatch");
          return undefined;
        }
        const message = new TextEncoder().encode(msgData.request.message);
        console.log("signing message with slush", {
          message,
          address,
          connectedAddress,
        });
        const signedMessage = await wallet?.features[
          "sui:signPersonalMessage"
        ].signPersonalMessage({
          message,
          account: connected?.accounts[0],
          chain: "sui:mainnet",
        });
        if (!signedMessage) {
          console.error("User rejected message");
          return undefined;
        }
        console.log("Slush Signed message", signedMessage);
        if (!signedMessage?.signature) {
          console.error("User rejected message");
          return undefined;
        }
        const request: LoginRequest = {
          ...msgData.request,
          signature: signedMessage?.signature,
        };
        return request;
      }
      case "phantom-solana": {
        const signedMessage = await signPhantomMessage({
          chain: "solana",
          message: msgData.request.message,
          display: "utf8",
        });
        const publicKey = (signedMessage as any)?.publicKey?.toString();
        console.log("Solana Public key", publicKey);
        const signature = (signedMessage as any)?.signature?.toString("hex");
        if (!signature) {
          console.error("User rejected message");
          return undefined;
        }
        const request: LoginRequest = {
          ...msgData.request,
          signature,
        };
        return request;
      }
      case "solflare-solana": {
        const signedMessage = await signSolflareMessage({
          message: msgData.request.message,
        });
        const signature = signedMessage?.signature;
        if (!signature) {
          console.error("User rejected message");
          return undefined;
        }
        const request: LoginRequest = {
          ...msgData.request,
          signature,
        };
        return request;
      }
      default:
        return undefined;
    }
  } catch (error) {
    console.error("signWalletMessage error:", error);
    throw error;
  }
}
