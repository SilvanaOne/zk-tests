"use client";

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
  console.log("NEXT_PUBLIC_LOCAL", process.env.NEXT_PUBLIC_LOCAL);
  const endpoint =
    process.env.NEXT_PUBLIC_LOCAL === "true"
      ? process.env.NEXT_PUBLIC_SILVANA_TEE_LOGIN_ENPOINT_LOCAL
      : process.env.NEXT_PUBLIC_SILVANA_TEE_LOGIN_ENPOINT_AWS;
  if (endpoint === undefined) {
    return {
      success: false,
      data: null,
      error: "NEXT_PUBLIC_SILVANA_TEE_LOGIN_ENPOINT is not set",
      indexes: null,
    };
  }
  try {
    console.log("Login request", request);
    console.log("endpoint", endpoint);
    const response = await fetch(endpoint, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ payload: request }),
      //body: JSON.stringify({ payload: { memo: "hi from client" } }),
    });
    console.timeEnd("Login request");
    if (!response.ok) {
      console.error("Login error:", response);
      return {
        success: false,
        data: null,
        error: `HTTP error! status: ${response.status}`,
        indexes: null,
      };
    }

    const data: EncryptedLoginResponse =
      process.env.NEXT_PUBLIC_LOCAL === "true"
        ? ((await response.json()) as EncryptedLoginResponse)
        : ((await response.json())?.response?.data as EncryptedLoginResponse);
    console.log("Login response:", data);
    if (
      data.success &&
      data.data !== null &&
      data.data !== undefined &&
      Array.isArray(data.data)
    ) {
      return data;
    } else {
      return {
        success: false,
        data: null,
        indexes: null,
        error: data.error,
      };
    }
  } catch (error: any) {
    console.error("Login error:", error);
    return {
      success: false,
      data: null,
      indexes: null,
      error: `Login error: ${error?.message}`,
    };
  }
}
