"use client";

import { generateKeyPair, decrypt } from "./encrypt";
import { rust_recover_mnemonic } from "./precompiles";

interface LoginRequest {
  chain: string;
  wallet: string;
  message: string;
  signature: string;
  address: string;
  public_key?: string;
  share_indexes?: number[];
}

interface LoginResponse {
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

export async function login(params: LoginRequest): Promise<LoginResponse> {
  let { publicKey, privateKey } = await generateKeyPair();
  if (privateKey === null) {
    return {
      success: false,
      seed: null,
      error: "Failed to generate key pair",
    };
  }
  try {
    params.public_key = publicKey;
    params.share_indexes = params.share_indexes || choose_shares_indexes();
    console.time("Login request");
    const response = await fetch("http://127.0.0.1:8000/login/", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(params),
    });
    console.timeEnd("Login request");
    if (!response.ok) {
      console.error("Login error:", response);
      privateKey = null;
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
          privateKey,
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
      privateKey = null;

      const seed = await rust_recover_mnemonic(shares);
      return {
        success: true,
        seed,
        error: null,
        indexes: data.indexes,
      };
    } else {
      privateKey = null;
      return {
        success: false,
        seed: null,
        error: data.error,
      };
    }
  } catch (error: any) {
    console.error("Login error:", error);
    privateKey = null;
    return {
      success: false,
      seed: null,
      error: `Login error: ${error?.message}`,
    };
  }
}
