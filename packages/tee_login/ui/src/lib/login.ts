"use client";

import { generateKeyPair, decrypt } from "./encrypt";
import { rust_recover_mnemonic } from "./precompiles";

export interface UnsignedLoginRequest {
  login_type: "wallet" | "social";
  chain: string;
  wallet: string;
  message: string;
  address: string;
  public_key: string;
  nonce: number;
  share_indexes?: number[];
}

export interface LoginRequest extends UnsignedLoginRequest {
  signature: string;
}

export interface LoginResponse {
  success: boolean;
  seed: string | null;
  error: string | null;
  indexes?: number[] | null;
}

interface EncryptedLoginResponse {
  success: boolean;
  data: string[] | null;
  error: string | null;
  indexes: number[] | null;
}

function choose_shares_indexes(): number[] {
  // Generate array of numbers from 0 to 15
  const available_numbers = Array.from({ length: 16 }, (_, i) => i);
  const chosen_indexes: number[] = [];

  // Randomly select exactly 10 numbers
  for (let i = 0; i < 10; i++) {
    const randomIndex = Math.floor(Math.random() * available_numbers.length);
    chosen_indexes.push(available_numbers[randomIndex]);
    // Remove the selected number to avoid duplicates
    available_numbers.splice(randomIndex, 1);
  }

  return chosen_indexes.sort((a, b) => a - b); // Sort for consistency
}

async function hashToBase64(data: string): Promise<string> {
  const encoder = new TextEncoder();
  const dataBytes = encoder.encode(data);
  const hashBuffer = await crypto.subtle.digest("SHA-256", dataBytes);
  const hashArray = new Uint8Array(hashBuffer);
  console.log("hashArray", hashArray);
  const base64 = Buffer.from(hashArray).toString("base64");
  console.log("base64", base64);
  return base64;
}

export async function getMessage(params: {
  login_type: "wallet" | "social";
  chain: string;
  wallet: string;
  address: string;
}): Promise<{
  privateKey: CryptoKey;
  request: UnsignedLoginRequest;
} | null> {
  const { login_type, chain, wallet, address } = params;
  const nonce = Date.now();
  const domain = "https://login.silvana.dev";
  const { publicKey, privateKey } = await generateKeyPair();
  if (publicKey === null || privateKey === null) {
    return null;
  }

  const metadata = JSON.stringify({
    domain,
    login_type,
    chain,
    wallet,
    address,
    publicKey,
    nonce,
  });
  console.log("metadata", metadata);
  const request = await hashToBase64(metadata);
  const message = `Silvana TEE login request: ${request}`;
  const loginRequest: UnsignedLoginRequest = {
    login_type,
    chain,
    wallet,
    address,
    message,
    public_key: publicKey,
    nonce,
    share_indexes: choose_shares_indexes(),
  };

  return {
    request: loginRequest,
    privateKey,
  };
}

export async function login(params: {
  request: LoginRequest;
  privateKey: CryptoKey;
}): Promise<LoginResponse> {
  const { request, privateKey } = params;
  if (privateKey === null) {
    return {
      success: false,
      seed: null,
      error: "Failed to generate key pair",
    };
  }
  const endpoint = process.env.NEXT_PUBLIC_SILVANA_TEE_LOGIN_ENPOINT;
  if (endpoint === undefined) {
    return {
      success: false,
      seed: null,
      error: "NEXT_PUBLIC_SILVANA_TEE_LOGIN_ENPOINT is not set",
    };
  }
  try {
    console.time("Login request");
    const response = await fetch(endpoint, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      //body: JSON.stringify(params.request),
      body: JSON.stringify({ memo: "Hi" }),
    });
    console.timeEnd("Login request");
    if (!response.ok) {
      console.error("Login error:", response);
      return {
        success: false,
        seed: null,
        error: `HTTP error! status: ${response.status}`,
      };
    }

    const data: EncryptedLoginResponse = await response.json();
    console.log("Login response:", data);
    if (
      data.success &&
      data.data !== null &&
      data.data !== undefined &&
      Array.isArray(data.data)
    ) {
      const shares: Uint8Array[] = [];
      for (const share of data.data) {
        const shareDecrypted = await decrypt({
          encrypted: share,
          privateKey: params.privateKey,
        });
        if (shareDecrypted === null) {
          return {
            success: false,
            seed: null,
            error: "Failed to decrypt share",
          };
        }
        shares.push(shareDecrypted);
      }

      const seed = await rust_recover_mnemonic(shares);
      return {
        success: true,
        seed,
        error: null,
        indexes: data.indexes,
      };
    } else {
      return {
        success: false,
        seed: null,
        error: data.error,
      };
    }
  } catch (error: any) {
    console.error("Login error:", error);
    return {
      success: false,
      seed: null,
      error: `Login error: ${error?.message}`,
    };
  }
}
