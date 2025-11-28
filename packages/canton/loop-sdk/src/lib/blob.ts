/**
 * Canton Contract Blob Decoder
 *
 * Utilities for fetching and decoding Canton contract data.
 * Uses the Lighthouse API to get decoded contract details since the
 * createdEventBlob from Loop SDK is a protobuf-encoded binary that
 * requires the Daml-LF value encoding schema to decode.
 *
 * The Lighthouse API returns the decoded create_arguments which contains
 * the contract payload (amount, parties, expiration dates, etc.)
 */

export type LoopNetwork = "devnet" | "testnet" | "mainnet";

// Lighthouse API base URLs per network
const LIGHTHOUSE_API_URLS: Record<LoopNetwork, string> = {
  devnet: "https://lighthouse.devnet.cantonloop.com/api",
  testnet: "https://lighthouse.testnet.cantonloop.com/api",
  mainnet: "https://lighthouse.cantonloop.com/api",
};

/**
 * Contract details returned from Lighthouse API
 */
export interface ContractDetails {
  id: string;
  template_id: string;
  package_name: string;
  choice: string;
  choice_argument: unknown | null;
  create_arguments: Record<string, unknown>;
}

/**
 * Amulet contract create_arguments structure
 */
export interface AmuletCreateArguments {
  dso: string;
  owner: string;
  amount: {
    initialAmount: string;
    createdAt: {
      number: string;
    };
    ratePerRound: {
      rate: string;
    };
  };
}

/**
 * TransferPreapproval contract create_arguments structure
 */
export interface TransferPreapprovalCreateArguments {
  dso: string;
  provider: string;
  receiver: string;
  expiresAt: string;
  lastRenewedAt: string;
  validFrom: string;
}

/**
 * TransferPreapprovalProposal contract create_arguments structure
 */
export interface TransferPreapprovalProposalCreateArguments {
  provider: string;
  receiver: string;
  expectedDso: string | null;
}

/**
 * Fetches contract details from Lighthouse API
 *
 * @param contractId - The full contract ID
 * @param network - The network to query (devnet, testnet, mainnet)
 * @returns Contract details including decoded create_arguments
 */
export async function fetchContractDetails(
  contractId: string,
  network: LoopNetwork = "devnet"
): Promise<ContractDetails | null> {
  const baseUrl = LIGHTHOUSE_API_URLS[network];
  const url = `${baseUrl}/contracts/${contractId}`;

  try {
    const response = await fetch(url);

    if (!response.ok) {
      console.error(`[blob.ts] Failed to fetch contract ${contractId}: ${response.status}`);
      return null;
    }

    const data = await response.json();
    return data as ContractDetails;
  } catch (error) {
    console.error(`[blob.ts] Error fetching contract ${contractId}:`, error);
    return null;
  }
}

/**
 * Fetches and parses Amulet contract details
 */
export async function fetchAmuletContract(
  contractId: string,
  network: LoopNetwork = "devnet"
): Promise<{ contract: ContractDetails; args: AmuletCreateArguments } | null> {
  const contract = await fetchContractDetails(contractId, network);

  if (!contract) return null;

  if (!contract.template_id.includes("Splice.Amulet:Amulet")) {
    console.warn(`[blob.ts] Contract ${contractId} is not an Amulet contract`);
    return null;
  }

  return {
    contract,
    args: contract.create_arguments as unknown as AmuletCreateArguments,
  };
}

/**
 * Fetches and parses TransferPreapproval contract details
 */
export async function fetchTransferPreapprovalContract(
  contractId: string,
  network: LoopNetwork = "devnet"
): Promise<{ contract: ContractDetails; args: TransferPreapprovalCreateArguments } | null> {
  const contract = await fetchContractDetails(contractId, network);

  if (!contract) return null;

  if (!contract.template_id.includes("TransferPreapproval") ||
      contract.template_id.includes("Proposal")) {
    console.warn(`[blob.ts] Contract ${contractId} is not a TransferPreapproval contract`);
    return null;
  }

  return {
    contract,
    args: contract.create_arguments as unknown as TransferPreapprovalCreateArguments,
  };
}

/**
 * Fetches multiple contracts in parallel
 */
export async function fetchContractsDetails(
  contractIds: string[],
  network: LoopNetwork = "devnet"
): Promise<Map<string, ContractDetails>> {
  const results = new Map<string, ContractDetails>();

  const promises = contractIds.map(async (contractId) => {
    const details = await fetchContractDetails(contractId, network);
    if (details) {
      results.set(contractId, details);
    }
  });

  await Promise.all(promises);
  return results;
}

/**
 * Calculates current Amulet amount considering decay rate
 *
 * Amulet uses ExpiringAmount which decays over time based on rounds.
 * Formula: currentAmount = initialAmount - (currentRound - createdAtRound) * ratePerRound
 *
 * @param amount - The ExpiringAmount structure from Amulet contract
 * @param currentRound - The current round number (optional, will estimate if not provided)
 * @returns The current amount as a string
 */
export function calculateCurrentAmuletAmount(
  amount: AmuletCreateArguments["amount"],
  currentRound?: number
): string {
  const initialAmount = parseFloat(amount.initialAmount);
  const createdAtRound = parseInt(amount.createdAt.number, 10);
  const ratePerRound = parseFloat(amount.ratePerRound.rate);

  // If currentRound not provided, estimate based on time
  // Rounds are approximately 2.5 seconds each on devnet
  if (currentRound === undefined) {
    // Just return initial amount if we can't calculate
    return amount.initialAmount;
  }

  const roundsElapsed = currentRound - createdAtRound;
  const decayedAmount = roundsElapsed * ratePerRound;
  const currentAmount = Math.max(0, initialAmount - decayedAmount);

  return currentAmount.toFixed(10);
}

/**
 * Formats a party ID for display
 * Shows the identifier part and truncated fingerprint
 *
 * @param partyId - Full party ID (e.g., "alice::1220abc123...")
 * @param truncate - Whether to truncate the fingerprint
 * @returns Formatted party ID string
 */
export function formatPartyId(partyId: string, truncate = true): string {
  const parts = partyId.split("::");
  if (parts.length !== 2) return partyId;

  const [identifier, fingerprint] = parts;

  if (!truncate || fingerprint.length <= 16) {
    return partyId;
  }

  return `${identifier}::${fingerprint.slice(0, 8)}...${fingerprint.slice(-8)}`;
}

/**
 * Checks if a party ID is the DSO party
 */
export function isDsoParty(partyId: string): boolean {
  return partyId.startsWith("DSO::");
}

/**
 * Extracts the identifier part from a party ID
 *
 * @param partyId - Full party ID (e.g., "alice::1220abc123...")
 * @returns The identifier part (e.g., "alice")
 */
export function getPartyIdentifier(partyId: string): string {
  return partyId.split("::")[0] || partyId;
}
