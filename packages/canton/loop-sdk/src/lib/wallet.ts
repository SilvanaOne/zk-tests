"use client";
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
  // Loop wallet (Canton network)
  {
    id: "loop-canton",
    name: "Loop",
    chain: "canton",
    logo: "/loop.svg",
    type: "wallet",
    description: "Canton Network",
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
    case "loop-canton":
      // Loop wallet connection is handled via Loop SDK
      // This function returns undefined as Loop uses a different connection flow
      return undefined;
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
      case "loop-canton":
        // Loop wallet signing is handled via Loop SDK
        // This is a placeholder - actual signing uses apiFunctions.signMessage
        return undefined;
      default:
        return undefined;
    }
  } catch (error) {
    console.error("signWalletMessage error:", error);
    throw error;
  }
}
