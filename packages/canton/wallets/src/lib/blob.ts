/**
 * Canton Contract Blob Decoder
 *
 * Utilities for fetching and decoding Canton contract data.
 * Uses protobuf decoding for createdEventBlob and Lighthouse API as fallback.
 *
 * The createdEventBlob contains a FatContractInstance protobuf message which
 * includes the contract arguments encoded as a Value.Record.
 */

import { fromBinary } from "@bufbuild/protobuf";
import { FatContractInstanceSchema, VersionedSchema } from "../proto/com/digitalasset/daml/lf/transaction_pb";
import { ValueSchema, type Value, type Value_Record } from "../proto/com/digitalasset/daml/lf/value_pb";

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
 * CIP-56 Holding contract create_arguments structure (Utility Registry Holding)
 */
export interface CIP56HoldingCreateArguments {
  operator: string;
  provider: string;
  registrar: string;
  owner: string;
  instrument: {
    admin: string;
    id: string;
    version: string;
  };
  label: string;
  amount: string;
  lock: unknown | null;
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

/**
 * Helper to extract string value from a Value
 */
function extractStringValue(value: Value | undefined): string {
  if (!value) return "";
  switch (value.sum.case) {
    case "party":
    case "text":
    case "numeric":
      return value.sum.value;
    default:
      return "";
  }
}

/**
 * Helper to extract record fields from a Value
 */
function extractRecord(value: Value | undefined): Value_Record | null {
  if (!value || value.sum.case !== "record") return null;
  return value.sum.value;
}

/**
 * Decoded blob result for any contract
 */
export interface DecodedContractBlob {
  contractId: Uint8Array;
  packageName: string;
  templateId: {
    packageId: string;
    moduleName: string[];
    name: string[];
  } | null;
  fields: Value_Record | null;
}

/**
 * Decodes a createdEventBlob from Loop SDK into a structured object
 *
 * @param base64Blob - The base64-encoded createdEventBlob
 * @returns Decoded contract data or null if decoding fails
 */
export function decodeContractBlob(base64Blob: string): DecodedContractBlob | null {
  try {
    // Base64 decode to Uint8Array
    const binaryString = atob(base64Blob);
    const binaryData = Uint8Array.from(binaryString, c => c.charCodeAt(0));

    // The blob is wrapped in a Versioned message (version + payload)
    const versioned = fromBinary(VersionedSchema, binaryData);

    // Parse the payload as FatContractInstance
    const fatContract = fromBinary(FatContractInstanceSchema, versioned.payload);

    // Parse create_arg directly as Value (NOT wrapped in VersionedValue)
    const value = fromBinary(ValueSchema, fatContract.createArg);
    const fields = extractRecord(value);

    return {
      contractId: fatContract.contractId,
      packageName: fatContract.packageName,
      templateId: fatContract.templateId ? {
        packageId: fatContract.templateId.packageId,
        moduleName: fatContract.templateId.moduleName,
        name: fatContract.templateId.name,
      } : null,
      fields,
    };
  } catch (error) {
    console.error("[blob.ts] Failed to decode contract blob:", error);
    return null;
  }
}

/**
 * Decodes a CIP-56 Holding contract from its createdEventBlob
 *
 * Supports two template types:
 *
 * 1. Utility.Registry.Holding.V0.Holding:Holding fields (in order):
 *    0: operator (Party)
 *    1: provider (Party)
 *    2: registrar (Party)
 *    3: owner (Party)
 *    4: instrument (Record: admin, id, version)
 *    5: label (Text)
 *    6: amount (Decimal/Numeric)
 *    7: lock (Optional Lock)
 *
 * 2. Splice.Amulet:Amulet fields (in order):
 *    0: dso (Party)
 *    1: owner (Party)
 *    2: amount (Record: initialAmount, createdAt, ratePerRound)
 *
 * @param base64Blob - The base64-encoded createdEventBlob
 * @returns Decoded CIP-56 holding arguments or null if decoding fails
 */
export function decodeCIP56HoldingBlob(base64Blob: string): CIP56HoldingCreateArguments | null {
  try {
    const decoded = decodeContractBlob(base64Blob);
    if (!decoded || !decoded.fields) {
      console.error("[blob.ts] Failed to decode CIP-56 holding: no fields");
      return null;
    }

    const fields = decoded.fields.fields;
    const templateName = decoded.templateId?.name?.join(":") || "";

    console.log("[blob.ts] Decoding template:", templateName, "fields count:", fields.length);

    // Check if this is an Amulet contract (implements Holding interface but different structure)
    if (templateName === "Amulet" || decoded.packageName === "splice-amulet") {
      // Amulet fields: 0=dso, 1=owner, 2=amount (ExpiringAmount record)
      if (fields.length < 3) {
        console.error(`[blob.ts] Amulet has insufficient fields: ${fields.length}`);
        return null;
      }

      const dso = extractStringValue(fields[0]?.value);
      const owner = extractStringValue(fields[1]?.value);

      // Extract ExpiringAmount record (field 2)
      const amountRecord = extractRecord(fields[2]?.value);
      let amount = "0";
      if (amountRecord && amountRecord.fields.length >= 1) {
        amount = extractStringValue(amountRecord.fields[0]?.value); // initialAmount
      }

      return {
        operator: dso,
        provider: dso,
        registrar: dso,
        owner,
        instrument: { admin: dso, id: "CC", version: "0" }, // Canton Coin
        label: "Canton Coin",
        amount,
        lock: null,
      };
    }

    // Standard CIP-56 Utility.Registry.Holding structure
    if (fields.length < 7) {
      console.error(`[blob.ts] CIP-56 holding has insufficient fields: ${fields.length}`);
      return null;
    }

    // Extract instrument record (field 4)
    const instrumentRecord = extractRecord(fields[4]?.value);
    let instrument = { admin: "", id: "", version: "" };
    if (instrumentRecord && instrumentRecord.fields.length >= 3) {
      instrument = {
        admin: extractStringValue(instrumentRecord.fields[0]?.value),
        id: extractStringValue(instrumentRecord.fields[1]?.value),
        version: extractStringValue(instrumentRecord.fields[2]?.value),
      };
    }

    // Extract lock (field 7) - Optional
    let lock: unknown | null = null;
    if (fields.length > 7 && fields[7]?.value?.sum.case === "optional") {
      const optionalValue = fields[7].value.sum.value;
      if (optionalValue.value) {
        // Lock is present, extract it
        lock = optionalValue.value;
      }
    }

    return {
      operator: extractStringValue(fields[0]?.value),
      provider: extractStringValue(fields[1]?.value),
      registrar: extractStringValue(fields[2]?.value),
      owner: extractStringValue(fields[3]?.value),
      instrument,
      label: extractStringValue(fields[5]?.value),
      amount: extractStringValue(fields[6]?.value),
      lock,
    };
  } catch (error) {
    console.error("[blob.ts] Failed to decode CIP-56 holding blob:", error);
    return null;
  }
}
