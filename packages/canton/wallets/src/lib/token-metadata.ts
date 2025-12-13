"use client";

import { getCurrentNetwork } from "./loop";

// Token metadata from utilities API
export interface TokenMetadata {
  instrumentId: string;
  instrumentAdmin: string;
  name: string;
  symbol: string;
  description?: string;
  logoUrl?: string;
  decimals?: number;
}

// Instrument configuration from utilities API
interface InstrumentConfiguration {
  instrument: {
    id: string;
    admin: string;
  };
  metadata: {
    name: string;
    symbol: string;
    description?: string;
    logo?: string;
    decimals?: number;
  };
}

// Cache for token metadata (keyed by instrumentAdmin::instrumentId)
const tokenMetadataCache = new Map<string, TokenMetadata>();

/**
 * Get utilities API URL based on network
 */
function getUtilitiesApiUrl(): string {
  const network = getCurrentNetwork();
  switch (network) {
    case "devnet":
      return "https://api.utilities.digitalasset-dev.com";
    case "testnet":
      return "https://api.utilities.digitalasset-test.com";
    case "mainnet":
      return "https://api.utilities.digitalasset.com";
    default:
      return "https://api.utilities.digitalasset-dev.com";
  }
}

/**
 * Fetch all instrument configurations from utilities API
 */
export async function fetchAllTokenMetadata(): Promise<TokenMetadata[]> {
  try {
    const apiUrl = getUtilitiesApiUrl();
    const response = await fetch(
      `${apiUrl}/api/utilities/v0/contract/instrument-configuration/all`,
      {
        method: "GET",
        headers: {
          "Accept": "application/json",
        },
      }
    );

    if (!response.ok) {
      console.error("[token-metadata] Failed to fetch instrument configurations:", response.status);
      return [];
    }

    const data = await response.json();
    console.log("[token-metadata] Fetched instrument configurations:", data);

    // Parse the response - it may be an array or have a nested structure
    const configurations: InstrumentConfiguration[] = Array.isArray(data)
      ? data
      : data.configurations || data.items || [];

    const metadata: TokenMetadata[] = configurations.map((config) => {
      const key = `${config.instrument.admin}::${config.instrument.id}`;
      const tokenMeta: TokenMetadata = {
        instrumentId: config.instrument.id,
        instrumentAdmin: config.instrument.admin,
        name: config.metadata.name,
        symbol: config.metadata.symbol,
        description: config.metadata.description,
        logoUrl: config.metadata.logo,
        decimals: config.metadata.decimals,
      };

      // Cache it
      tokenMetadataCache.set(key, tokenMeta);

      return tokenMeta;
    });

    return metadata;
  } catch (error) {
    console.error("[token-metadata] Error fetching token metadata:", error);
    return [];
  }
}

/**
 * Get token metadata from cache by instrument admin and ID
 */
export function getTokenMetadata(
  instrumentAdmin: string,
  instrumentId: string
): TokenMetadata | undefined {
  const key = `${instrumentAdmin}::${instrumentId}`;
  return tokenMetadataCache.get(key);
}

/**
 * Get token metadata from cache, or fetch all if cache is empty
 */
export async function getOrFetchTokenMetadata(
  instrumentAdmin: string,
  instrumentId: string
): Promise<TokenMetadata | undefined> {
  // Check cache first
  let metadata = getTokenMetadata(instrumentAdmin, instrumentId);
  if (metadata) {
    return metadata;
  }

  // Fetch all metadata if cache is empty
  if (tokenMetadataCache.size === 0) {
    await fetchAllTokenMetadata();
    metadata = getTokenMetadata(instrumentAdmin, instrumentId);
  }

  return metadata;
}

/**
 * Clear the token metadata cache
 */
export function clearTokenMetadataCache(): void {
  tokenMetadataCache.clear();
}

/**
 * Get all cached token metadata
 */
export function getAllCachedTokenMetadata(): TokenMetadata[] {
  return Array.from(tokenMetadataCache.values());
}
