// src/utils.ts
// Example utility module for cryptographic operations

/**
 * Securely zeroes out a Uint8Array
 * @param arr - Array to zero out
 */
export function secureZero(arr: Uint8Array): void {
  if (arr && arr.fill) {
    arr.fill(0);
  }
}

/**
 * Converts base64 string to Uint8Array
 * @param b64 - Base64 encoded string
 * @returns Decoded bytes
 */
export function b64ToBytes(b64: string): Uint8Array {
  return Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
}

/**
 * Converts Uint8Array to hex string
 * @param arr - Byte array
 * @returns Hex encoded string
 */
export function bytesToHex(arr: Uint8Array): string {
  return [...arr].map((b) => b.toString(16).padStart(2, "0")).join("");
}

/**
 * Converts hex string to Uint8Array
 * @param hex - Hex encoded string
 * @returns Byte array
 */
export function hexToBytes(hex: string): Uint8Array {
  if (hex.length % 2 !== 0) {
    throw new Error("Invalid hex string length");
  }

  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.substr(i, 2), 16);
  }
  return bytes;
}

/**
 * Generates a random request ID
 * @returns Random UUID string
 */
export function uuid(): string {
  if (typeof crypto !== "undefined" && crypto.randomUUID) {
    return crypto.randomUUID();
  }

  // Fallback for environments without crypto.randomUUID
  return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, function (c) {
    const r = (Math.random() * 16) | 0;
    const v = c === "x" ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}
