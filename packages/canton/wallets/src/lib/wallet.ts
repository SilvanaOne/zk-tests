"use client";
import { WalletChain, WalletProvider, WalletType } from "./types";
import { connectPhantom } from "./phantom";

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
  provider: WalletProvider;
}

export type WalletOption = WalletOptionWallet | WalletOptionSocial;

export const walletOptions: WalletOption[] = [
  // Loop wallet (Canton network)
  {
    id: "loop-canton",
    name: "Loop",
    chain: "canton",
    logo: "/loop.png",
    type: "wallet",
    description: "Canton Network",
  },
  // Phantom wallet (Solana)
  {
    id: "phantom-solana",
    name: "Phantom",
    chain: "solana",
    logo: "/phantom.svg",
    type: "wallet",
    description: "Solana",
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
      return undefined;
    case "phantom-solana":
      return connectPhantom();
    default:
      throw new Error(`Unsupported wallet id: ${walletId}`);
  }
}
