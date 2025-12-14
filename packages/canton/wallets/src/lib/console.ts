"use client";

/**
 * Console Wallet integration
 * Uses @console-wallet/dapp-sdk for browser extension wallet connection
 */

import { consoleWalletPixelplex, CANTON_NETWORK_VARIANTS } from "@console-wallet/dapp-sdk";

// Store the connected account info
let consoleAccount: {
  partyId: string;
  publicKey: string;
  hint: string;
  namespace: string;
  networkId: string;
  status: string;
  signingProviderId: string;
  primary: boolean;
} | null = null;

// Console account type
export interface ConsoleAccountInfo {
  partyId: string;
  publicKey: string;
  hint: string;
  namespace: string;
  networkId: string;
  status: string;
  signingProviderId: string;
  primary: boolean;
}

/**
 * Connect to Console Wallet browser extension.
 * Opens the wallet popup for user approval.
 * @returns Account info with all wallet details, or null on failure
 */
export async function connectConsoleWallet(): Promise<ConsoleAccountInfo | null> {
  console.log("[console.ts] Connecting to Console Wallet...");

  try {
    // Request connection from Console Wallet extension
    // Note: icon must be an absolute URL for the extension to load it
    const iconUrl = typeof window !== "undefined"
      ? `${window.location.origin}/loop.png`
      : "/loop.png";

    const connectResult = await consoleWalletPixelplex.connect({
      name: "Silvana Wallet",
      icon: iconUrl,
    });

    console.log("[console.ts] Connect result:", connectResult);

    if (!connectResult) {
      console.log("[console.ts] Connection rejected or failed");
      return null;
    }

    // Get the primary/active account
    const account = await consoleWalletPixelplex.getPrimaryAccount();

    console.log("[console.ts] Primary account:", account);

    if (!account || !account.partyId) {
      console.log("[console.ts] No account or partyId available");
      return null;
    }

    consoleAccount = {
      partyId: account.partyId,
      publicKey: account.publicKey,
      hint: account.hint || "",
      namespace: account.namespace || "",
      networkId: account.networkId || "",
      status: account.status || "",
      signingProviderId: account.signingProviderId || "",
      primary: account.primary || false,
    };

    console.log("[console.ts] Console Wallet connected:", consoleAccount.partyId, "hint:", consoleAccount.hint);
    return consoleAccount;
  } catch (error) {
    console.error("[console.ts] Connection error:", error);
    return null;
  }
}

/**
 * Disconnect from Console Wallet.
 * Clears local state and notifies the extension.
 */
export async function disconnectConsoleWallet(): Promise<void> {
  console.log("[console.ts] Disconnecting Console Wallet...");

  try {
    await consoleWalletPixelplex.disconnect();
  } catch (error) {
    console.error("[console.ts] Disconnect error:", error);
  }

  consoleAccount = null;
  console.log("[console.ts] Console Wallet disconnected");
}

/**
 * Get the current party ID from Console Wallet.
 * @returns The party ID if connected, null otherwise
 */
export function getConsolePartyId(): string | null {
  return consoleAccount?.partyId || null;
}

/**
 * Get the current public key from Console Wallet.
 * @returns The public key if connected, null otherwise
 */
export function getConsolePublicKey(): string | null {
  return consoleAccount?.publicKey || null;
}

/**
 * Get the wallet hint/name from Console Wallet.
 * @returns The hint if connected, null otherwise
 */
export function getConsoleHint(): string | null {
  return consoleAccount?.hint || null;
}

/**
 * Get the namespace from Console Wallet.
 * @returns The namespace if connected, null otherwise
 */
export function getConsoleNamespace(): string | null {
  return consoleAccount?.namespace || null;
}

/**
 * Get the network ID from Console Wallet.
 * @returns The network ID if connected, null otherwise
 */
export function getConsoleNetworkId(): string | null {
  return consoleAccount?.networkId || null;
}

/**
 * Get all Console Wallet account info.
 * @returns Full account info if connected, null otherwise
 */
export function getConsoleAccountInfo(): ConsoleAccountInfo | null {
  return consoleAccount;
}

/**
 * Check if Console Wallet is connected.
 * @returns true if connected
 */
export async function isConsoleWalletConnected(): Promise<boolean> {
  try {
    const status = await consoleWalletPixelplex.status();
    return !!status;
  } catch {
    return false;
  }
}

/**
 * Subscribe to account changes from Console Wallet.
 * @param callback Function to call when account changes
 */
export function onConsoleAccountsChanged(callback: (partyId: string | null) => void): void {
  consoleWalletPixelplex.onAccountsChanged((account) => {
    if (account && account.partyId) {
      consoleAccount = {
        partyId: account.partyId,
        publicKey: account.publicKey,
        hint: account.hint || "",
        namespace: account.namespace || "",
        networkId: account.networkId || "",
        status: account.status || "",
        signingProviderId: account.signingProviderId || "",
        primary: account.primary || false,
      };
      callback(account.partyId);
    } else {
      consoleAccount = null;
      callback(null);
    }
  });
}

/**
 * Subscribe to connection status changes from Console Wallet.
 * @param callback Function to call when connection status changes
 */
export function onConsoleConnectionStatusChanged(callback: (connected: boolean) => void): void {
  consoleWalletPixelplex.onConnectionStatusChanged((status) => {
    console.log("[console.ts] Connection status changed:", status);
    callback(!!status);
  });
}

// Console holding type for balance display
export interface ConsoleHolding {
  tokenId: string;
  tokenName: string;
  amount: string;
  amountBigInt: string;
  decimals: number;
  price: string;
  balanceUsd?: string;
  imageSrc: string;
  network: string;
}

/**
 * Map app network to Console Wallet network variant.
 */
function getConsoleNetworkVariant(network: string): CANTON_NETWORK_VARIANTS {
  switch (network) {
    case "devnet":
      return CANTON_NETWORK_VARIANTS.CANTON_NETWORK_DEV;
    case "testnet":
      return CANTON_NETWORK_VARIANTS.CANTON_NETWORK_TEST;
    case "mainnet":
      return CANTON_NETWORK_VARIANTS.CANTON_NETWORK;
    default:
      return CANTON_NETWORK_VARIANTS.CANTON_NETWORK_DEV;
  }
}

/**
 * Get CC and token balances from Console Wallet.
 * Returns balances for supported coins: CC, CBTC, USDCx
 * @param partyId The party identifier
 * @param network The network to query (devnet/testnet/mainnet)
 * @returns Array of holdings with balance info
 */
export async function getConsoleHoldings(
  partyId: string,
  network: "devnet" | "testnet" | "mainnet"
): Promise<ConsoleHolding[]> {
  console.log("[console.ts] Fetching holdings for party:", partyId, "network:", network);

  try {
    const networkVariant = getConsoleNetworkVariant(network);
    const result = await consoleWalletPixelplex.getCoinsBalance({
      party: partyId,
      network: networkVariant,
    });

    console.log("[console.ts] getCoinsBalance result:", result);

    if (!result || !result.tokens) {
      console.log("[console.ts] No tokens returned");
      return [];
    }

    return result.tokens.map((token) => ({
      tokenId: token.symbol,
      tokenName: token.name,
      amount: token.balance,
      amountBigInt: token.balanceBigInt,
      decimals: token.decimals,
      price: token.price,
      balanceUsd: token.balanceUsd,
      imageSrc: token.imageSrc,
      network: token.network,
    }));
  } catch (error) {
    console.error("[console.ts] Error fetching holdings:", error);
    return [];
  }
}
