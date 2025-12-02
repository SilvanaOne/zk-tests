"use client";

import { signSolflareTransactionHash } from "./solflare";
import { prepareInteractiveSubmission, executeInteractiveSubmission } from "./interactive-submission-actions";
import { getAmuletContractsFromLedger } from "./ledger-api";
import type { TransferResult } from "./loop";

// Type for the transfer context response from /api/scan/transfer-context
interface DisclosedContractInfo {
  contractId: string;
  templateId: string;
  createdEventBlob: string;
}

interface TransferContext {
  amuletRulesContractId: string;
  openRoundContractId: string;
  transferPreapprovalContractId: string;
  featuredAppRightContractId: string | null;
  externalPartyAmuletRulesContractId: string;
  dsoParty: string;
  amuletRules: DisclosedContractInfo;
  openMiningRound: DisclosedContractInfo;
  transferPreapproval: DisclosedContractInfo;
  featuredAppRight: DisclosedContractInfo | null;
  externalPartyAmuletRules: DisclosedContractInfo;
  synchronizerId: string;
}

/**
 * Fetches transfer context from Scan API via our Next.js server route
 */
async function fetchTransferContext(receiverParty: string): Promise<TransferContext> {
  const response = await fetch(
    `/api/scan/transfer-context?network=devnet&receiverParty=${encodeURIComponent(receiverParty)}`
  );

  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.error || `Failed to fetch transfer context: ${response.status}`);
  }

  return response.json();
}

/**
 * Transfer CC using Solflare wallet with Canton's interactive submission flow.
 *
 * This uses ExternalPartyAmuletRules.TransferFactory_Transfer which only requires
 * sender authorization (unlike TransferPreapproval_Send which requires both
 * sender and provider signatures).
 *
 * Flow:
 * 1. Fetch transfer context from Scan API (amuletRules, openRound, preapproval, etc.)
 * 2. Get sender's Amulet contract IDs from Ledger API
 * 3. Build ExerciseCommand with all disclosed contracts
 * 4. Prepare transaction via Canton interactive submission
 * 5. Sign transaction hash with Solflare wallet
 * 6. Execute signed transaction
 */
export async function createSolflareTransfer(params: {
  senderPartyId: string;
  receiverPartyId: string;
  amount: string;
  description?: string;
}): Promise<TransferResult> {
  const { senderPartyId, receiverPartyId, amount, description } = params;

  console.log("[solflare-transfer] Starting transfer:", {
    sender: senderPartyId,
    receiver: receiverPartyId,
    amount,
    description,
  });

  try {
    // 1. Fetch transfer context from Scan API
    console.log("[solflare-transfer] Fetching transfer context for receiver:", receiverPartyId);
    const context = await fetchTransferContext(receiverPartyId);
    console.log("[solflare-transfer] Transfer context received:", {
      amuletRulesContractId: context.amuletRulesContractId?.substring(0, 30) + "...",
      openRoundContractId: context.openRoundContractId?.substring(0, 30) + "...",
      externalPartyAmuletRulesContractId: context.externalPartyAmuletRulesContractId?.substring(0, 30) + "...",
      dsoParty: context.dsoParty?.substring(0, 30) + "...",
      synchronizerId: context.synchronizerId?.substring(0, 30) + "...",
    });

    // 2. Get sender's Amulet contracts from Ledger API
    console.log("[solflare-transfer] Fetching sender's Amulet contracts...");
    const amuletContracts = await getAmuletContractsFromLedger(senderPartyId);

    if (!amuletContracts || amuletContracts.length === 0) {
      return { success: false, error: "No Amulet holdings found. You need CC to make a transfer." };
    }

    // Extract contract IDs
    const inputHoldingCids = amuletContracts.map(c => c.contractId);
    console.log("[solflare-transfer] Found", inputHoldingCids.length, "Amulet contracts");

    // 3. Build timestamps for the transfer
    const now = new Date();
    const executeBefore = new Date(now.getTime() + 30000); // 30 seconds from now

    // Format timestamps to match DAML expected format
    const requestedAtStr = now.toISOString().replace(/Z$/, '000Z'); // Add 000 for microseconds
    const executeBeforeStr = executeBefore.toISOString(); // Keep millisecond precision

    // 4. Build disclosed contracts array - 5 contracts total
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

    // 5. Build the TransferFactory_Transfer command
    const commands = [{
      ExerciseCommand: {
        templateId: "#splice-api-token-transfer-instruction-v1:Splice.Api.Token.TransferInstructionV1:TransferFactory",
        contractId: context.externalPartyAmuletRulesContractId,
        choice: "TransferFactory_Transfer",
        choiceArgument: {
          expectedAdmin: context.dsoParty,
          transfer: {
            sender: senderPartyId,
            receiver: receiverPartyId,
            amount: amount.includes('.') ? amount : `${amount}.0`,
            instrumentId: {
              admin: context.dsoParty,
              id: "Amulet"
            },
            inputHoldingCids,
            requestedAt: requestedAtStr,
            executeBefore: executeBeforeStr,
            meta: {
              values: {
                "splice.lfdecentralizedtrust.org/reason": description || "Transfer"
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
    }];

    console.log("[solflare-transfer] Built TransferFactory_Transfer command");
    console.log("[solflare-transfer] Command:", JSON.stringify(commands, null, 2));

    // 6. Prepare transaction via Canton interactive submission
    console.log("[solflare-transfer] Preparing transaction...");
    const prepareResult = await prepareInteractiveSubmission({
      externalPartyId: senderPartyId,
      commands,
      disclosedContracts,
    });

    if (!prepareResult.success) {
      console.error("[solflare-transfer] Prepare failed:", prepareResult.error);
      return { success: false, error: prepareResult.error || "Failed to prepare transaction" };
    }

    const { preparedTransaction, preparedTransactionHash, hashingSchemeVersion } = prepareResult;
    console.log("[solflare-transfer] Transaction prepared:", {
      hasTransaction: !!preparedTransaction,
      hashLength: preparedTransactionHash?.length,
      hashingScheme: hashingSchemeVersion,
    });

    // 7. Sign transaction hash with Solflare wallet
    console.log("[solflare-transfer] Requesting Solflare signature...");
    const signatureBase64 = await signSolflareTransactionHash(preparedTransactionHash!);

    if (!signatureBase64) {
      return { success: false, error: "User rejected signing or signature failed" };
    }

    console.log("[solflare-transfer] Signature obtained, length:", signatureBase64.length);

    // 8. Execute the signed transaction
    console.log("[solflare-transfer] Executing signed transaction...");
    const executeResult = await executeInteractiveSubmission({
      preparedTransaction: preparedTransaction!,
      preparedTransactionHash: preparedTransactionHash!,
      hashingSchemeVersion: hashingSchemeVersion!,
      externalPartyId: senderPartyId,
      signatureBase64,
    });

    if (!executeResult.success) {
      console.error("[solflare-transfer] Execute failed:", executeResult.error);
      return { success: false, error: executeResult.error || "Failed to execute transaction" };
    }

    console.log("[solflare-transfer] Transfer completed successfully:", {
      updateId: executeResult.updateId,
      submissionId: executeResult.submissionId,
    });

    return {
      success: true,
      submissionId: executeResult.submissionId,
      commandId: executeResult.updateId,
    };
  } catch (error) {
    console.error("[solflare-transfer] Error:", error);
    return {
      success: false,
      error: error instanceof Error ? error.message : "Transfer failed"
    };
  }
}
