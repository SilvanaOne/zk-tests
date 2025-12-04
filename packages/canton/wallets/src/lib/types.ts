import type React from "react";

export type WalletChain = "canton" | "solana";
export type WalletProvider = "google" | "github";
export type WalletType = "wallet" | "social";

export interface SocialLoginData {
  id: string;
  name?: string;
  email?: string;
  idToken?: string;
  accessToken?: string;
  expires: string;
}

export interface UserStatus {
  loginType: WalletType;
  walletId?: string;
  address?: string;
  isConnected: boolean;
  isConnectionFailed: boolean;
  isConnecting: boolean;
  publicKey?: string;
  icon?: React.ElementType;
}

export interface UserWalletStatus extends UserStatus {
  loginType: "wallet";
  chain: WalletChain;
  wallet: string;
}

export interface UserSocialLoginStatus extends UserStatus {
  loginType: "social";
  provider: WalletProvider;
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
}

export interface UnifiedUserState {
  connections: Record<string, UserConnectionStatus>;
  selectedAuthMethod: UserConnectionStatus | null;
}
