"use client";
import { loop, LoopProvider, type Holding, type ActiveContract } from "@fivenorth/loop-sdk";

export type { ActiveContract };

export type LoopNetwork = "devnet" | "testnet" | "mainnet";

// Use window object to store provider to avoid module instance duplication issues in Next.js
declare global {
  interface Window {
    __loopProvider?: LoopProvider | null;
    __loopNetwork?: LoopNetwork;
    __loopCallbacks?: {
      onConnect?: (provider: LoopProvider) => void;
      onReject?: () => void;
    };
  }
}

function getProvider(): LoopProvider | null {
  if (typeof window === "undefined") return null;
  return window.__loopProvider ?? null;
}

function setProvider(provider: LoopProvider | null) {
  if (typeof window !== "undefined") {
    window.__loopProvider = provider;
  }
}

function getNetwork(): LoopNetwork {
  if (typeof window === "undefined") return "devnet";
  return window.__loopNetwork ?? "devnet";
}

function setNetwork(network: LoopNetwork) {
  if (typeof window !== "undefined") {
    window.__loopNetwork = network;
  }
}

function getCallbacks() {
  if (typeof window === "undefined") return {};
  return window.__loopCallbacks ?? {};
}

function setCallbacks(callbacks: { onConnect?: (provider: LoopProvider) => void; onReject?: () => void }) {
  if (typeof window !== "undefined") {
    window.__loopCallbacks = callbacks;
  }
}

export function initLoop(
  callbacks: { onConnect?: (provider: LoopProvider) => void; onReject?: () => void },
  network: LoopNetwork = "devnet"
) {
  setCallbacks(callbacks);
  setNetwork(network);

  loop.init({
    appName: "Silvana Wallet Connect",
    network: network,
    onAccept: (provider) => {
      console.log("[loop.ts] onAccept called with provider:", provider);
      setProvider(provider);
      getCallbacks().onConnect?.(provider);
    },
    onReject: () => {
      getCallbacks().onReject?.();
    },
  });
}

export function connectLoop(network?: LoopNetwork) {
  const currentNetwork = getNetwork();
  if (network && network !== currentNetwork) {
    // Clear old ticket when switching networks to force new ticket generation
    if (typeof window !== "undefined") {
      localStorage.removeItem("loop_connect");
    }
    setNetwork(network);
    loop.init({
      appName: "Silvana Wallet Connect",
      network: network,
      onAccept: (provider: LoopProvider) => {
        console.log("[loop.ts] connectLoop onAccept called with provider:", provider);
        setProvider(provider);
        getCallbacks().onConnect?.(provider);
      },
      onReject: () => {
        getCallbacks().onReject?.();
      },
    });
  }
  loop.connect();
}

export function getLoopProvider(): LoopProvider | null {
  return getProvider();
}

export function getCurrentNetwork(): LoopNetwork {
  return getNetwork();
}

export function disconnectLoop() {
  // Clear local storage to logout (Loop SDK stores session in 'loop_connect')
  if (typeof window !== "undefined") {
    localStorage.removeItem("loop_connect");
  }
  setProvider(null);
}

export async function getLoopHoldings(): Promise<Holding[] | null> {
  const provider = getProvider();
  console.log("[loop.ts] getLoopHoldings called, loopProvider:", provider);
  if (!provider) {
    console.log("[loop.ts] loopProvider is null, cannot fetch holdings");
    return null;
  }
  return provider.getHolding();
}

export async function getLoopActiveContracts(
  params?: { templateId?: string; interfaceId?: string }
): Promise<ActiveContract[] | null> {
  const provider = getProvider();
  console.log("[loop.ts] getLoopActiveContracts called, loopProvider:", provider);
  if (!provider) {
    console.log("[loop.ts] loopProvider is null, cannot fetch active contracts");
    return null;
  }
  return provider.getActiveContracts(params);
}

export async function signLoopMessage(message: string): Promise<any> {
  const provider = getProvider();
  console.log("[loop.ts] signLoopMessage called, message:", message);
  if (!provider) {
    console.log("[loop.ts] loopProvider is null, cannot sign message");
    return null;
  }
  return provider.signMessage(message);
}

export function getLoopPublicKey(): string | null {
  return getProvider()?.public_key ?? null;
}

/**
 * Verifies that a partyId's namespace is cryptographically derived from the public key.
 *
 * Canton Party ID format: identifier::1220fingerprint
 * Where fingerprint = SHA256(0x0000000C || public_key_bytes)
 * The 1220 prefix is multihash encoding (0x12=SHA-256, 0x20=32 bytes)
 */
export async function verifyPartyIdMatchesPublicKey(
  partyId: string,
  publicKeyHex: string
): Promise<boolean> {
  try {
    // Parse partyId: identifier::1220fingerprint
    const parts = partyId.split("::");
    if (parts.length !== 2) {
      console.log("[loop.ts] Invalid partyId format - expected identifier::namespace");
      return false;
    }

    const namespace = parts[1];

    // Namespace should start with 1220 (multihash prefix for SHA-256)
    if (!namespace.startsWith("1220")) {
      console.log("[loop.ts] Invalid namespace format - expected 1220 prefix");
      return false;
    }

    const expectedFingerprint = namespace.slice(4); // Remove 1220 prefix

    // Compute fingerprint: SHA256(purpose_id_4bytes || public_key_bytes)
    // HashPurpose.PublicKeyFingerprint = 12 (decimal) = 0x0000000C (4 bytes big-endian)
    const purposeBuffer = new ArrayBuffer(4);
    const purposeBytes = new Uint8Array(purposeBuffer);
    purposeBytes.set([0x00, 0x00, 0x00, 0x0c]);
    const publicKeyBytes = hexToBytes(publicKeyHex);

    // Concatenate purpose + public key
    const inputBuffer = new ArrayBuffer(purposeBytes.length + publicKeyBytes.length);
    const input = new Uint8Array(inputBuffer);
    input.set(purposeBytes, 0);
    input.set(publicKeyBytes, purposeBytes.length);

    // Compute SHA-256
    const hashBuffer = await crypto.subtle.digest("SHA-256", input);
    const hashArray = new Uint8Array(hashBuffer);
    const computedFingerprint = Array.from(hashArray)
      .map(b => b.toString(16).padStart(2, "0"))
      .join("");

    const isValid = computedFingerprint === expectedFingerprint;
    console.log("[loop.ts] PartyId verification:", {
      expected: expectedFingerprint,
      computed: computedFingerprint,
      isValid
    });

    return isValid;
  } catch (error) {
    console.error("[loop.ts] PartyId verification error:", error);
    return false;
  }
}

// Helper function to convert hex string to bytes
function hexToBytes(hex: string): Uint8Array<ArrayBuffer> {
  const buffer = new ArrayBuffer(hex.length / 2);
  const bytes = new Uint8Array(buffer);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.slice(i, i + 2), 16);
  }
  return bytes;
}

export async function verifyLoopSignature(
  message: string,
  signatureHex: string
): Promise<boolean> {
  const publicKeyHex = getLoopPublicKey();
  if (!publicKeyHex) {
    console.log("[loop.ts] No public key available for verification");
    return false;
  }

  // Parse signature if it's JSON (e.g., {"signature":"..."})
  let sigHex = signatureHex;
  try {
    const parsed = JSON.parse(signatureHex);
    if (parsed.signature) {
      sigHex = parsed.signature;
    }
  } catch {
    // Not JSON, use as-is
  }

  try {
    const publicKey = await crypto.subtle.importKey(
      "raw",
      hexToBytes(publicKeyHex),
      { name: "Ed25519" },
      false,
      ["verify"]
    );

    const isValid = await crypto.subtle.verify(
      "Ed25519",
      publicKey,
      hexToBytes(sigHex),
      new TextEncoder().encode(message)
    );

    console.log("[loop.ts] Signature verification result:", isValid);
    return isValid;
  } catch (error) {
    console.error("[loop.ts] Signature verification error:", error);
    return false;
  }
}

// Disclosed contract info from Scan API
interface DisclosedContractInfo {
  contractId: string;
  templateId: string;
  createdEventBlob: string;
}

// Transfer context response from the Scan API
export interface TransferContext {
  amuletRulesContractId: string;
  openRoundContractId: string;
  transferPreapprovalContractId: string;
  featuredAppRightContractId: string | null;
  externalPartyAmuletRulesContractId: string;
  dsoParty: string;
  // Full contract info for disclosed contracts
  amuletRules: DisclosedContractInfo;
  openMiningRound: DisclosedContractInfo;
  transferPreapproval: DisclosedContractInfo;
  featuredAppRight: DisclosedContractInfo | null;
  externalPartyAmuletRules: DisclosedContractInfo;
  synchronizerId: string;
}

// Transfer result structure
export interface TransferResult {
  success: boolean;
  updateId?: string;         // Canton Ledger API updateId (1220... format)
  submissionId?: string;     // Loop-specific submission ID
  commandId?: string;        // Loop-specific command ID
  error?: string;
}

// Preapproval result structure
export interface PreapprovalResult {
  success: boolean;
  updateId?: string;
  submissionId?: string;
  commandId?: string;
  error?: string;
}

// Hardcoded constants for devnet
const DEVNET_DSO_PARTY = "DSO::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a";
const ORDERBOOK_OPERATOR_PARTY = "orderbook-operator-1::122034faf8f4af71d107a42441f8bc90cabfd63ab4386fc7f17d15d6e3b01c5bd2ae";

/**
 * Extracts the Canton updateId from Loop's base64-encoded transaction_data.
 * The transaction_data is a protobuf-encoded Transaction or TransactionTree message.
 * update_id is field 1 (string) in the protobuf schema.
 *
 * Canton updateId format: 1220 + 64 hex chars (multihash SHA-256)
 *
 * Protobuf wire format for field 1 string:
 * - Tag byte: 0x0a (field 1, wire type 2 = length-delimited)
 * - Length: varint encoding
 * - Value: UTF-8 string bytes
 */
function extractUpdateId(transactionDataBase64: string): string | undefined {
  try {
    // Decode base64 to binary
    const binaryString = atob(transactionDataBase64);
    const bytes = new Uint8Array(binaryString.length);
    for (let i = 0; i < binaryString.length; i++) {
      bytes[i] = binaryString.charCodeAt(i);
    }

    // Parse protobuf field 1 (update_id)
    // The message starts with tag 0x0a (field 1, wire type 2)
    let pos = 0;

    // Read tag byte - should be 0x0a for field 1, wire type 2 (length-delimited)
    if (bytes[pos] !== 0x0a) {
      console.warn("[loop.ts] Expected protobuf tag 0x0a at position 0, got:", bytes[pos]?.toString(16));
      return undefined;
    }
    pos++;

    // Read varint length
    let length = 0;
    let shift = 0;
    while (pos < bytes.length) {
      const b = bytes[pos++];
      length |= (b & 0x7f) << shift;
      if ((b & 0x80) === 0) break;
      shift += 7;
    }

    // Read the string value
    if (pos + length > bytes.length) {
      console.warn("[loop.ts] Protobuf string length exceeds buffer");
      return undefined;
    }

    const updateIdBytes = bytes.slice(pos, pos + length);
    const updateId = new TextDecoder('utf-8').decode(updateIdBytes);

    // Validate it matches Canton updateId format: 1220 + 64 hex chars
    if (/^1220[a-f0-9]{64}$/i.test(updateId)) {
      return updateId;
    }

    console.warn("[loop.ts] Extracted field 1 doesn't match updateId format:", updateId);
    return undefined;
  } catch (e) {
    console.warn("[loop.ts] Could not extract updateId from transaction_data:", e);
    return undefined;
  }
}

/**
 * Fetches transfer context from Scan API via our Next.js server route
 */
async function fetchTransferContext(
  receiverParty: string,
  network: LoopNetwork
): Promise<TransferContext> {
  const response = await fetch(
    `/api/scan/transfer-context?network=${network}&receiverParty=${encodeURIComponent(receiverParty)}`
  );

  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.error || `Failed to fetch transfer context: ${response.status}`);
  }

  return response.json();
}

/**
 * Transfers CC (Canton Coin/Amulet) to another party using TransferFactory_Transfer
 *
 * Uses the ExternalPartyAmuletRules contract which implements the TransferFactory interface.
 * This approach only requires sender authorization (unlike TransferPreapproval_Send which
 * requires both sender and provider authorization).
 *
 * @param params.receiver - The receiver's partyId
 * @param params.amount - Amount to transfer (as a decimal string, e.g., "100.0")
 * @param params.description - Optional description/reason for the transfer
 */
export async function transferCC(params: {
  receiver: string;
  amount: string;
  description?: string;
}): Promise<TransferResult> {
  const provider = getProvider();
  if (!provider) {
    return { success: false, error: "Not connected to Loop wallet" };
  }

  const network = getNetwork();
  const sender = provider.party_id;

  console.log("[loop.ts] transferCC called:", { sender, ...params, network });

  try {
    // 1. Fetch transfer context from Scan API
    console.log("[loop.ts] Fetching transfer context for receiver:", params.receiver);
    const context = await fetchTransferContext(params.receiver, network);
    console.log("[loop.ts] Transfer context:", context);

    // 2. Get user's Amulet holdings to use as inputs
    const holdings = await provider.getActiveContracts({
      templateId: "#splice-amulet:Splice.Amulet:Amulet"
    });

    if (!holdings || holdings.length === 0) {
      return { success: false, error: "No Amulet holdings found. You need CC to make a transfer." };
    }

    console.log("[loop.ts] Found", holdings.length, "Amulet contracts");

    // 3. Extract contract IDs from holdings
    const inputHoldingCids = holdings
      .map(h => h.contractEntry?.JsActiveContract?.createdEvent?.contractId)
      .filter((id): id is string => !!id);

    if (inputHoldingCids.length === 0) {
      return { success: false, error: "Could not extract contract IDs from Amulet holdings" };
    }

    console.log("[loop.ts] Prepared", inputHoldingCids.length, "input contracts for transfer");

    // 4. Build timestamps for the transfer
    // requestedAt uses microsecond precision (6 digits), executeBefore uses millisecond precision (3 digits)
    const now = new Date();
    const executeBefore = new Date(now.getTime() + 30000); // 30 seconds from now

    // Format timestamps to match DAML expected format
    // toISOString() gives: 2025-11-27T21:04:42.104Z (3 digits milliseconds)
    // For requestedAt, we need 6 digits: 2025-11-27T21:04:42.104000Z
    // For executeBefore, we need 3 digits: 2025-11-27T21:05:12.104Z (as-is)
    const requestedAtStr = now.toISOString().replace(/Z$/, '000Z'); // Add 000 for microseconds
    const executeBeforeStr = executeBefore.toISOString(); // Keep millisecond precision

    // 5. Build disclosed contracts array - 5 contracts total
    // These are contracts the sender doesn't have visibility to but needs to reference
    const disclosedContracts: Array<{
      contractId: string;
      createdEventBlob: string;
      synchronizerId: string;
      templateId: string;
    }> = [
      // AmuletRules
      {
        contractId: context.amuletRules.contractId,
        createdEventBlob: context.amuletRules.createdEventBlob,
        synchronizerId: context.synchronizerId,
        templateId: context.amuletRules.templateId,
      },
      // OpenMiningRound
      {
        contractId: context.openMiningRound.contractId,
        createdEventBlob: context.openMiningRound.createdEventBlob,
        synchronizerId: context.synchronizerId,
        templateId: context.openMiningRound.templateId,
      },
      // ExternalPartyAmuletRules (the TransferFactory contract)
      {
        contractId: context.externalPartyAmuletRules.contractId,
        createdEventBlob: context.externalPartyAmuletRules.createdEventBlob,
        synchronizerId: context.synchronizerId,
        templateId: context.externalPartyAmuletRules.templateId,
      },
      // TransferPreapproval
      {
        contractId: context.transferPreapproval.contractId,
        createdEventBlob: context.transferPreapproval.createdEventBlob,
        synchronizerId: context.synchronizerId,
        templateId: context.transferPreapproval.templateId,
      },
    ];

    // Add FeaturedAppRight if available
    if (context.featuredAppRight) {
      disclosedContracts.push({
        contractId: context.featuredAppRight.contractId,
        createdEventBlob: context.featuredAppRight.createdEventBlob,
        synchronizerId: context.synchronizerId,
        templateId: context.featuredAppRight.templateId,
      });
    }

    // 6. Build the TransferFactory_Transfer command
    const command = {
      commands: [{
        ExerciseCommand: {
          templateId: "#splice-api-token-transfer-instruction-v1:Splice.Api.Token.TransferInstructionV1:TransferFactory",
          contractId: context.externalPartyAmuletRulesContractId,
          choice: "TransferFactory_Transfer",
          choiceArgument: {
            expectedAdmin: context.dsoParty,
            transfer: {
              sender,
              receiver: params.receiver,
              amount: params.amount.includes('.') ? params.amount : `${params.amount}.0`,
              instrumentId: {
                admin: context.dsoParty,
                id: "Amulet"
              },
              inputHoldingCids,
              requestedAt: requestedAtStr,
              executeBefore: executeBeforeStr,
              meta: {
                values: {
                  "splice.lfdecentralizedtrust.org/reason": params.description || "Transfer"
                }
              }
            },
            extraArgs: {
              context: {
                values: {
                  "amulet-rules": {
                    tag: "AV_ContractId",
                    value: context.amuletRulesContractId
                  },
                  "open-round": {
                    tag: "AV_ContractId",
                    value: context.openRoundContractId
                  },
                  "featured-app-right": {
                    tag: "AV_ContractId",
                    value: context.featuredAppRightContractId
                  },
                  "transfer-preapproval": {
                    tag: "AV_ContractId",
                    value: context.transferPreapprovalContractId
                  }
                }
              },
              meta: { values: {} }
            }
          }
        }
      }],
      disclosedContracts,
    };

    console.log("[loop.ts] Submitting TransferFactory_Transfer command:", JSON.stringify(command, null, 2));

    // 7. Submit the transaction
    const result = await provider.submitTransaction(command);
    console.log("[loop.ts] Transfer result:", result);

    // 8. Extract Canton updateId from transaction_data
    let updateId: string | undefined;
    if (result?.transaction_data) {
      updateId = extractUpdateId(result.transaction_data);
      console.log("[loop.ts] Extracted updateId:", updateId);
    }

    return {
      success: true,
      updateId,
      submissionId: result?.submission_id,
      commandId: result?.command_id,
    };
  } catch (error) {
    console.error("[loop.ts] Transfer error:", error);
    return {
      success: false,
      error: error instanceof Error ? error.message : "Transfer failed"
    };
  }
}

/**
 * Creates a TransferPreapprovalProposal contract.
 *
 * This is Step 1 of the preapproval process:
 * 1. Receiver creates TransferPreapprovalProposal (this function)
 * 2. Provider accepts the proposal (separate process)
 *
 * The logged-in user becomes the receiver who will be able to receive
 * transfers from the specified provider once the proposal is accepted.
 *
 * @param params.provider - The provider's partyId (e.g., orderbook-operator-1::...)
 *                          Defaults to ORDERBOOK_OPERATOR_PARTY if not specified
 */
export async function createTransferPreapprovalProposal(params?: {
  provider?: string;
}): Promise<PreapprovalResult> {
  const loopProvider = getProvider();
  if (!loopProvider) {
    return { success: false, error: "Not connected to Loop wallet" };
  }

  const receiver = loopProvider.party_id;
  const provider = params?.provider || ORDERBOOK_OPERATOR_PARTY;

  console.log("[loop.ts] createTransferPreapprovalProposal called:", { receiver, provider });

  try {
    // Build the CreateCommand for TransferPreapprovalProposal
    const command = {
      commands: [{
        CreateCommand: {
          templateId: "#splice-wallet:Splice.Wallet.TransferPreapproval:TransferPreapprovalProposal",
          createArguments: {
            provider,
            receiver,
            expectedDso: DEVNET_DSO_PARTY
          }
        }
      }],
      // No disclosedContracts needed for CreateCommand
      disclosedContracts: []
    };

    console.log("[loop.ts] Submitting TransferPreapprovalProposal command:", JSON.stringify(command, null, 2));

    // Submit the transaction
    const result = await loopProvider.submitTransaction(command);
    console.log("[loop.ts] Preapproval proposal result:", result);

    // Extract Canton updateId from transaction_data
    let updateId: string | undefined;
    if (result?.transaction_data) {
      updateId = extractUpdateId(result.transaction_data);
      console.log("[loop.ts] Extracted updateId:", updateId);
    }

    return {
      success: true,
      updateId,
      submissionId: result?.submission_id,
      commandId: result?.command_id,
    };
  } catch (error) {
    console.error("[loop.ts] Preapproval proposal error:", error);
    return {
      success: false,
      error: error instanceof Error ? error.message : "Failed to create preapproval proposal"
    };
  }
}
