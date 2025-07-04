"use client";
import { Logger } from "@logtail/next";
import { teeApiCall } from "./tee";
const log = new Logger({
  source: "login",
});

export interface UnsignedLoginRequest {
  login_type: "wallet" | "social";
  chain: string;
  wallet: string;
  message: string;
  address: string;
  public_key: string;
  nonce: number;
  share_indexes: number[];
}

export interface LoginRequest extends UnsignedLoginRequest {
  signature: string;
}

export interface LoginResponse {
  success: boolean;
  publicKey: string | null;
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
  const base64 = Buffer.from(hashArray).toString("base64");
  return base64;
}

export async function getMessage(params: {
  login_type: "wallet" | "social";
  chain: string;
  wallet: string;
  address: string;
  publicKey: string;
}): Promise<{
  request: UnsignedLoginRequest;
} | null> {
  const { login_type, chain, wallet, address, publicKey } = params;
  const nonce = Date.now();
  const domain = "https://login.silvana.dev";

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
  };
}

export async function login(
  request: LoginRequest
): Promise<EncryptedLoginResponse> {
  try {
    const response = await teeApiCall({
      endpoint: "login",
      request,
    });
    if (!response.success) {
      log.error("Login error T102:", {
        response,
      });
      return {
        success: false,
        data: null,
        error: response.error ?? "Error T102",
        indexes: null,
      };
    }

    const data: EncryptedLoginResponse =
      process.env.NEXT_PUBLIC_LOCAL === "true"
        ? (response.data as EncryptedLoginResponse)
        : ((response.data as any)?.response?.data as EncryptedLoginResponse);
    console.log("Login response:", data);
    if (
      data.success &&
      data.data !== null &&
      data.data !== undefined &&
      Array.isArray(data.data)
    ) {
      return data;
    } else {
      log.error("Login error T103:", {
        response: data.error,
      });
      return {
        success: false,
        data: null,
        indexes: null,
        error: data.error,
      };
    }
  } catch (error: any) {
    log.error("Login error T104:", {
      error,
    });
    return {
      success: false,
      data: null,
      indexes: null,
      error: `Login error: ${error?.message}`,
    };
  }
}
