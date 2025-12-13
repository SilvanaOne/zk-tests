"use client";

import { getLoopProvider } from "./loop";

export interface RequestServiceResult {
  success: boolean;
  contractId?: string;
  submissionId?: string;
  commandId?: string;
  error?: string;
}

// Hardcoded DSO party for devnet (same as in loop.ts)
const DEVNET_DSO_PARTY = "DSO::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a";

/**
 * Create a UserServiceRequest contract using Loop wallet's submitTransaction.
 * This follows the same pattern as Transfer CC - the user signs in their Loop wallet.
 */
export async function createUserServiceRequest(params: {
  userPartyId: string;
  operatorPartyId: string;
  walletType?: "loop" | "solflare";
}): Promise<RequestServiceResult> {
  const { userPartyId, operatorPartyId, walletType = "loop" } = params;

  console.log("[request-service] Creating UserServiceRequest");
  console.log("  - userPartyId:", userPartyId);
  console.log("  - operatorPartyId:", operatorPartyId);
  console.log("  - walletType:", walletType);

  // Currently only Loop wallet is supported for this pattern
  if (walletType !== "loop") {
    return { success: false, error: "Only Loop wallet is supported for Request Service" };
  }

  const provider = getLoopProvider();
  if (!provider) {
    return { success: false, error: "Not connected to Loop wallet" };
  }

  try {
    // Get package name from environment
    const packageName = process.env.NEXT_PUBLIC_UTILITY_CREDENTIAL_PACKAGE_NAME;
    if (!packageName) {
      return { success: false, error: "NEXT_PUBLIC_UTILITY_CREDENTIAL_PACKAGE_NAME not configured" };
    }

    const templateId = `#${packageName}:Utility.Credential.App.V0.Service.User:UserServiceRequest`;

    // Build the CreateCommand for UserServiceRequest
    // Similar to createTransferPreapprovalProposal in loop.ts
    const command = {
      commands: [{
        CreateCommand: {
          templateId,
          createArguments: {
            operator: operatorPartyId,
            user: userPartyId,
          },
        },
      }],
      // No disclosedContracts needed for CreateCommand
      disclosedContracts: [],
    };

    console.log("[request-service] Submitting UserServiceRequest command:", JSON.stringify(command, null, 2));

    // Submit the transaction via Loop wallet
    // This will show the Loop signing popup to the user
    const result = await provider.submitTransaction(command);
    console.log("[request-service] Submit result:", result);

    return {
      success: true,
      submissionId: result?.submission_id,
      commandId: result?.command_id,
    };
  } catch (error) {
    console.error("[request-service] Error:", error);
    return {
      success: false,
      error: error instanceof Error ? error.message : "Request failed",
    };
  }
}
