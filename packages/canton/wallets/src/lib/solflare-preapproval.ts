"use client";

import { signSolflareTransactionHash } from "./solflare";
import type { PreapprovalResult } from "./loop";
import {
  prepareInteractiveSubmission,
  executeInteractiveSubmission,
} from "./interactive-submission-actions";

// Hardcoded constants for devnet
const DEVNET_DSO_PARTY = "DSO::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a";
const DEFAULT_PROVIDER = "orderbook-operator-1::122034faf8f4af71d107a42441f8bc90cabfd63ab4386fc7f17d15d6e3b01c5bd2ae";

/**
 * Creates a TransferPreapprovalProposal for a Solflare wallet user.
 *
 * Uses Canton's interactive submission flow:
 * 1. Prepare - Get the transaction hash from the server
 * 2. Sign - Sign the hash with Solflare Solana wallet
 * 3. Execute - Submit the signed transaction
 *
 * Solflare does not have Phantom's security restriction that blocks
 * signing raw 32-byte binary data, making it work with Canton's hash signing.
 *
 * @param params.solflarePartyId - The Canton party ID mapped from the Solana public key
 * @param params.provider - Optional provider party ID (defaults to settlement operator)
 * @returns Result indicating success or failure
 */
export async function createSolflarePreapprovalProposal(params: {
  solflarePartyId: string;
  provider?: string;
}): Promise<PreapprovalResult> {
  const { solflarePartyId, provider = DEFAULT_PROVIDER } = params;

  console.log("[solflare-preapproval] Creating preapproval proposal:", {
    receiver: solflarePartyId,
    provider,
  });

  try {
    // Step 1: Build the CreateCommand for TransferPreapprovalProposal
    const commands = [{
      CreateCommand: {
        templateId: "#splice-wallet:Splice.Wallet.TransferPreapproval:TransferPreapprovalProposal",
        createArguments: {
          provider,
          receiver: solflarePartyId,
          expectedDso: DEVNET_DSO_PARTY,
        },
      },
    }];

    // Step 2: Call prepare server action to get the transaction hash
    console.log("[solflare-preapproval] Calling prepare server action...");
    const prepareResult = await prepareInteractiveSubmission({
      externalPartyId: solflarePartyId,
      commands,
      disclosedContracts: [],
    });

    if (!prepareResult.success || !prepareResult.preparedTransactionHash) {
      console.error("[solflare-preapproval] Prepare failed:", prepareResult.error);
      return {
        success: false,
        error: prepareResult.error || "Prepare failed",
      };
    }

    console.log("[solflare-preapproval] Prepare succeeded, hash length:", prepareResult.preparedTransactionHash.length);

    // Step 3: Sign the transaction hash with Solflare wallet
    console.log("[solflare-preapproval] Requesting Solflare signature...");
    const signatureBase64 = await signSolflareTransactionHash(prepareResult.preparedTransactionHash);

    if (!signatureBase64) {
      console.log("[solflare-preapproval] User rejected signature or signing failed");
      return {
        success: false,
        error: "Signature rejected or failed. Please try again.",
      };
    }

    console.log("[solflare-preapproval] Got signature, executing...");

    // Step 4: Call execute server action with the signed transaction
    const executeResult = await executeInteractiveSubmission({
      preparedTransaction: prepareResult.preparedTransaction!,
      preparedTransactionHash: prepareResult.preparedTransactionHash,
      hashingSchemeVersion: prepareResult.hashingSchemeVersion!,
      externalPartyId: solflarePartyId,
      signatureBase64,
    });

    if (!executeResult.success) {
      console.error("[solflare-preapproval] Execute failed:", executeResult.error);
      return {
        success: false,
        error: executeResult.error || "Execute failed",
      };
    }

    console.log("[solflare-preapproval] Preapproval proposal created successfully:", {
      updateId: executeResult.updateId,
      submissionId: executeResult.submissionId,
    });

    return {
      success: true,
      submissionId: executeResult.submissionId,
    };
  } catch (error: any) {
    console.error("[solflare-preapproval] Error:", error);
    return {
      success: false,
      error: error.message || "Failed to create preapproval proposal",
    };
  }
}
