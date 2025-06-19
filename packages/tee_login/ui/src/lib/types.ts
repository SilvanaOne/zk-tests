import type React from "react";
export interface Addresses {
  solana_address: string;
  sui_address: string;
  mina_address: string;
  ethereum_address: string;
}

export interface Attestation {
  is_valid: boolean;
  digest: string;
  timestamp: number;
  module_id: string;
  public_key?: Uint8Array; // Assuming it might be part of full data
  user_data?: Uint8Array; // Assuming it might be part of full data
  nonce?: Uint8Array; // Assuming it might be part of full data
  pcr_vec: string[];
  pcr_map: Record<number, string>;
  pcr_locked?: Record<number, boolean>; // Add locked status for each PCR
  addresses?: Addresses;
}

export interface TeeStats {
  cpu_cores: number;
  memory: number; // Total memory in bytes
  available_memory: number;
  free_memory: number; // This might be different from available_memory depending on OS
  used_memory: number;
  timestamp: string; // ISO date string
}

export interface TeeStatusData {
  stats: TeeStats;
  attestation: Attestation;
}

export interface UserStatus {
  loginType: "wallet" | "social";
  walletId?: string;
  address?: string;
  isConnected: boolean;
  isConnecting: boolean;
  minaPublicKey?: string;
  shamirShares?: number[]; // Array of share numbers used (1-16)
  icon?: React.ElementType;
}

export interface UserWalletStatus extends UserStatus {
  loginType: "wallet";
  chain: "ethereum" | "solana" | "sui";
  wallet: string;
}

export interface UserSocialLoginStatus extends UserStatus {
  loginType: "social";
  provider: "google" | "github";
  isLoggedIn: boolean;
  username?: string;
  email?: string;
  avatarUrl?: string;
  sessionExpires?: string;
}

export type UserConnectionStatus = UserWalletStatus | UserSocialLoginStatus;

export type WalletConnectionState =
  | "idle"
  | "connecting"
  | "connected"
  | "error";

export interface WalletConnectionResult {
  state: WalletConnectionState;
  error?: string;
  address?: string;
  shamirShares?: string[];
}

export interface ApiFunctions {
  getPrivateKeyId: () => Promise<{
    privateKeyId: string;
    publicKey: string;
  } | null>;
  decryptShares: (
    data: string[],
    privateKeyId: string
  ) => Promise<string | null>;
  signMessage: (params: {
    publicKey: string;
    message: string;
  }) => Promise<{ signature: string | null; error: string | null }>;
}

export interface UnifiedUserState {
  connections: Record<string, UserConnectionStatus>;
  selectedAuthMethod: UserConnectionStatus | null;
}
