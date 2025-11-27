"use client";
import { LoginRequest } from "./login";

export type TEE_ENDPOINT = "login" | "stats" | "get_attestation";

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

export interface TeeStatus {
  status: TeeStatusData | null;
  isLoading: boolean;
  sections?: string[];
  title?: string;
}

export async function teeApiCall(params: {
  endpoint: string;
  request?: LoginRequest;
}): Promise<{ success: boolean; data?: object; error?: string }> {
  const { endpoint, request } = params;
  const url_base =
    process.env.NEXT_PUBLIC_LOCAL === "true"
      ? process.env.NEXT_PUBLIC_SILVANA_TEE_LOGIN_ENPOINT_LOCAL
      : process.env.NEXT_PUBLIC_SILVANA_TEE_LOGIN_ENPOINT_AWS;
  if (url_base === undefined) {
    return {
      success: false,
      error: "NEXT_PUBLIC_SILVANA_TEE_LOGIN_ENPOINT is not set",
    };
  }
  const url = `${url_base}/${endpoint}`;
  if (endpoint === "login") {
    if (request === undefined) {
      return {
        success: false,
        error: "Request is required for login endpoint",
      };
    }
  }
  const id = uuid();
  try {
    console.time(`TEE API call ${endpoint} ${id}`);
    const response = await fetch(url, {
      method: endpoint === "login" ? "POST" : "GET",
      headers: {
        "Content-Type": "application/json",
      },
      body:
        endpoint === "login" ? JSON.stringify({ payload: request }) : undefined,
    });
    console.timeEnd(`TEE API call ${endpoint} ${id}`);
    if (!response.ok) {
      console.error(
        `TEE API call ${endpoint} error:`,
        response.statusText,
        response.status
      );
      return {
        success: false,
        error: `TEE API call ${endpoint} error: ${response.status} ${response.statusText}`,
      };
    }

    const data = await response.json();
    return {
      success: true,
      data,
    };
  } catch (error: any) {
    console.error(`TEE API call ${endpoint} error:`, error?.message);
    return {
      success: false,
      error: `TEE API call ${endpoint} error: ${
        error?.message ?? "Error T101"
      }`,
    };
  }
}

let attestationLoading = false;
let attestationResponse: AttestationResponse | null = null;
interface AttestationResponse {
  success: boolean;
  data?: string;
  error?: string;
}

export async function getAttestation(): Promise<AttestationResponse> {
  if (attestationResponse) {
    await sleep(1000);
    return attestationResponse;
  }
  if (attestationLoading) {
    while (attestationLoading) {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    if (attestationResponse) {
      await sleep(1000);
      return attestationResponse;
    }
  }
  attestationLoading = true;
  const response = await teeApiCall({
    endpoint: "get_attestation",
  });
  const data = (response as any)?.data?.attestation as string;
  const result = {
    success: response.success && data !== undefined && data !== null,
    data,
    error: response.error,
  };
  attestationResponse = result;
  attestationLoading = false;
  return result;
}

export async function getStats(): Promise<{
  success: boolean;
  data?: TeeStats;
  error?: string;
}> {
  const response = await teeApiCall({
    endpoint: "stats",
  });
  const data = (response as any)?.data?.response?.data as TeeStats;
  return {
    success: response.success && data !== undefined && data !== null,
    data,
    error: response.error,
  };
}

function uuid() {
  return crypto.randomUUID();
}

async function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
