"use client";

import { signSolflareTransactionHash } from "./solflare";
import { prepareInteractiveSubmission, executeInteractiveSubmission } from "./interactive-submission-actions";
import { getAmuletContractsFromLedger } from "./ledger-api";

// Result type for accept operation
export interface AcceptResult {
  success: boolean;
  submissionId?: string;
  updateId?: string;
  advancedPaymentCid?: string;
  error?: string;
}

// Type for the accept context response from /api/scan/accept-context
interface DisclosedContractInfo {
  contractId: string;
  templateId: string;
  createdEventBlob: string;
}

interface AcceptContext {
  amuletRulesContractId: string;
  openRoundContractId: string;
  featuredAppRightContractId: string | null;
  dsoParty: string;
  amuletRules: DisclosedContractInfo;
  openMiningRound: DisclosedContractInfo;
  featuredAppRight: DisclosedContractInfo | null;
  synchronizerId: string;
}

/**
 * Fetches accept context from Scan API via our Next.js server route
 */
async function fetchAcceptContext(providerHint?: string): Promise<AcceptContext> {
  const url = providerHint
    ? `/api/scan/accept-context?network=devnet&providerHint=${encodeURIComponent(providerHint)}`
    : `/api/scan/accept-context?network=devnet`;

  const response = await fetch(url);

  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.error || `Failed to fetch accept context: ${response.status}`);
  }

  return response.json();
}

/**
 * Accept an AdvancedPaymentRequest using Solflare wallet with Canton's interactive submission flow.
 *
 * This exercises the AdvancedPaymentRequest_Accept choice which:
 * 1. Takes the user's Amulet contracts as input
 * 2. Locks the required amount
 * 3. Creates an AdvancedPayment contract
 *
 * Flow:
 * 1. Fetch accept context from Scan API (amuletRules, openRound, etc.)
 * 2. Get sender's Amulet contract IDs from Ledger API
 * 3. Build ExerciseCommand with all disclosed contracts
 * 4. Prepare transaction via Canton interactive submission
 * 5. Sign transaction hash with Solflare wallet
 * 6. Execute signed transaction
 */
export async function acceptAdvancedPaymentRequest(params: {
  senderPartyId: string;
  requestContractId: string;
  packageId: string;
  providerHint?: string;
}): Promise<AcceptResult> {
  const { senderPartyId, requestContractId, packageId, providerHint } = params;

  console.log("=".repeat(80));
  console.log("[solflare-accept-request] Starting accept request");
  console.log("=".repeat(80));
  console.log("[solflare-accept-request] Parameters:");
  console.log("  - senderPartyId:", senderPartyId);
  console.log("  - requestContractId:", requestContractId);
  console.log("  - packageId:", packageId);
  console.log("  - providerHint:", providerHint || "(none)");

  try {
    // 1. Fetch accept context from Scan API
    console.log("\n[solflare-accept-request] Step 1: Fetching accept context...");
    const context = await fetchAcceptContext(providerHint);
    console.log("[solflare-accept-request] Accept context received:");
    console.log("  - amuletRulesContractId:", context.amuletRulesContractId?.substring(0, 40) + "...");
    console.log("  - openRoundContractId:", context.openRoundContractId?.substring(0, 40) + "...");
    console.log("  - featuredAppRightContractId:", context.featuredAppRightContractId?.substring(0, 40) + "..." || "null");
    console.log("  - dsoParty:", context.dsoParty?.substring(0, 40) + "...");
    console.log("  - synchronizerId:", context.synchronizerId?.substring(0, 40) + "...");

    // 2. Get sender's Amulet contracts from Ledger API
    console.log("\n[solflare-accept-request] Step 2: Fetching sender's Amulet contracts...");
    const amuletContracts = await getAmuletContractsFromLedger(senderPartyId);

    if (!amuletContracts || amuletContracts.length === 0) {
      console.error("[solflare-accept-request] No Amulet holdings found");
      return { success: false, error: "No Amulet holdings found. You need CC to accept this request." };
    }

    // Extract contract IDs (ownerInputs)
    const ownerInputs = amuletContracts.map(c => c.contractId);
    console.log("[solflare-accept-request] Found", ownerInputs.length, "Amulet contracts:");
    ownerInputs.forEach((cid, i) => {
      console.log(`  [${i}]: ${cid.substring(0, 40)}...`);
    });

    // 3. Build disclosed contracts array
    console.log("\n[solflare-accept-request] Step 3: Building disclosed contracts...");
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

    console.log("[solflare-accept-request] Disclosed contracts count:", disclosedContracts.length);
    disclosedContracts.forEach((dc, i) => {
      console.log(`  [${i}]: ${dc.templateId} - ${dc.contractId.substring(0, 30)}...`);
    });

    // 4. Build the AdvancedPaymentRequest_Accept command
    console.log("\n[solflare-accept-request] Step 4: Building exercise command...");
    const templateId = `${packageId}:AdvancedPaymentRequest:AdvancedPaymentRequest`;
    const appTransferContext = {
      amuletRules: context.amuletRulesContractId,
      openMiningRound: context.openRoundContractId,
      featuredAppRight: context.featuredAppRightContractId,
    };

    const commands = [{
      ExerciseCommand: {
        templateId,
        contractId: requestContractId,
        choice: "AdvancedPaymentRequest_Accept",
        choiceArgument: {
          ownerInputs,
          appTransferContext,
        },
      },
    }];

    console.log("[solflare-accept-request] Command payload:");
    console.log(JSON.stringify(commands, null, 2));

    // 5. Prepare transaction via Canton interactive submission
    console.log("\n[solflare-accept-request] Step 5: Preparing transaction...");
    const prepareResult = await prepareInteractiveSubmission({
      externalPartyId: senderPartyId,
      commands,
      disclosedContracts,
    });

    if (!prepareResult.success) {
      console.error("[solflare-accept-request] Prepare failed:", prepareResult.error);
      return { success: false, error: prepareResult.error || "Failed to prepare transaction" };
    }

    const { preparedTransaction, preparedTransactionHash, hashingSchemeVersion } = prepareResult;
    console.log("[solflare-accept-request] Transaction prepared successfully:");
    console.log("  - preparedTransaction length:", preparedTransaction?.length);
    console.log("  - preparedTransactionHash:", preparedTransactionHash);
    console.log("  - hashingSchemeVersion:", hashingSchemeVersion);

    // 6. Sign transaction hash with Solflare wallet
    console.log("\n[solflare-accept-request] Step 6: Requesting Solflare signature...");
    const signatureBase64 = await signSolflareTransactionHash(preparedTransactionHash!);

    if (!signatureBase64) {
      console.error("[solflare-accept-request] User rejected signing or signature failed");
      return { success: false, error: "User rejected signing or signature failed" };
    }

    console.log("[solflare-accept-request] Signature obtained:");
    console.log("  - signature length:", signatureBase64.length);
    console.log("  - signature (first 50 chars):", signatureBase64.substring(0, 50) + "...");

    // 7. Execute the signed transaction
    console.log("\n[solflare-accept-request] Step 7: Executing signed transaction...");
    const executeResult = await executeInteractiveSubmission({
      preparedTransaction: preparedTransaction!,
      preparedTransactionHash: preparedTransactionHash!,
      hashingSchemeVersion: hashingSchemeVersion!,
      externalPartyId: senderPartyId,
      signatureBase64,
    });

    if (!executeResult.success) {
      console.error("[solflare-accept-request] Execute failed:", executeResult.error);
      return { success: false, error: executeResult.error || "Failed to execute transaction" };
    }

    console.log("\n" + "=".repeat(80));
    console.log("[solflare-accept-request] Accept completed successfully!");
    console.log("=".repeat(80));
    console.log("  - submissionId:", executeResult.submissionId);
    console.log("  - updateId:", executeResult.updateId);

    return {
      success: true,
      submissionId: executeResult.submissionId,
      updateId: executeResult.updateId,
    };
  } catch (error) {
    console.error("[solflare-accept-request] Error:", error);
    return {
      success: false,
      error: error instanceof Error ? error.message : "Accept request failed",
    };
  }
}
