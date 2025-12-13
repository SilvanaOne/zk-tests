"use client";

import { getLoopProvider, getLoopActiveContracts, getCurrentNetwork } from "./loop";

export interface AcceptCredentialOfferResult {
  success: boolean;
  credentialCid?: string;
  credentialBillingCid?: string;
  submissionId?: string;
  commandId?: string;
  error?: string;
}

// App transfer context from Scan API
interface AppTransferContext {
  amuletRulesContractId: string;
  openRoundContractId: string;
  featuredAppRightContractId: string | null;
  amuletRules: { contractId: string; templateId: string; createdEventBlob: string };
  openMiningRound: { contractId: string; templateId: string; createdEventBlob: string };
  featuredAppRight: { contractId: string; templateId: string; createdEventBlob: string } | null;
  synchronizerId: string;
}

/**
 * Fetch app transfer context from our server-side API route.
 * This provides amuletRules, openMiningRound, and featuredAppRight contracts.
 * Uses /api/scan/accept-context which fetches from Scan API server-side (no CORS issues).
 */
async function fetchAppTransferContext(): Promise<AppTransferContext> {
  const network = getCurrentNetwork();

  console.log("[accept-credential-offer] Fetching app transfer context via /api/scan/accept-context");

  const response = await fetch(`/api/scan/accept-context?network=${network}`);

  if (!response.ok) {
    const error = await response.json().catch(() => ({ error: "Unknown error" }));
    throw new Error(error.error || `Failed to fetch accept context: ${response.status}`);
  }

  const data = await response.json();

  return {
    amuletRulesContractId: data.amuletRulesContractId,
    openRoundContractId: data.openRoundContractId,
    featuredAppRightContractId: data.featuredAppRightContractId,
    amuletRules: data.amuletRules,
    openMiningRound: data.openMiningRound,
    featuredAppRight: data.featuredAppRight,
    synchronizerId: data.synchronizerId,
  };
}

/**
 * Accept a CredentialOffer using Loop wallet's submitTransaction.
 * Uses UserService_AcceptPaidCredentialOffer for paid credentials.
 * Uses UserService_AcceptFreeCredentialOffer for free credentials.
 */
export async function acceptCredentialOffer(params: {
  userPartyId: string;
  userServiceCid: string;
  credentialOfferCid: string;
  isPaid: boolean;
  depositAmountUsd?: number;  // Required deposit amount in USD for paid credentials
}): Promise<AcceptCredentialOfferResult> {
  const { userPartyId, userServiceCid, credentialOfferCid, isPaid, depositAmountUsd } = params;

  console.log("[accept-credential-offer] Accepting CredentialOffer");
  console.log("  - userPartyId:", userPartyId);
  console.log("  - userServiceCid:", userServiceCid);
  console.log("  - credentialOfferCid:", credentialOfferCid);
  console.log("  - isPaid:", isPaid);
  console.log("  - depositAmountUsd:", depositAmountUsd);

  const provider = getLoopProvider();
  if (!provider) {
    return { success: false, error: "Not connected to Loop wallet" };
  }

  try {
    const packageName = process.env.NEXT_PUBLIC_UTILITY_CREDENTIAL_PACKAGE_NAME;
    if (!packageName) {
      return { success: false, error: "NEXT_PUBLIC_UTILITY_CREDENTIAL_PACKAGE_NAME not configured" };
    }

    const templateId = `#${packageName}:Utility.Credential.App.V0.Service.User:UserService`;

    let command;
    if (isPaid) {
      // For paid credentials, we need:
      // 1. User's unlocked Amulet contracts (depositAmulets)
      // 2. App transfer context (amuletRules, openMiningRound, featuredAppRight)

      console.log("[accept-credential-offer] Fetching user's Amulet contracts...");

      // Get user's Amulet holdings
      const amuletContracts = await getLoopActiveContracts({
        templateId: "#splice-amulet:Splice.Amulet:Amulet"
      });

      if (!amuletContracts || amuletContracts.length === 0) {
        return { success: false, error: "No Amulet holdings found. You need CC to accept a paid credential offer." };
      }

      console.log("[accept-credential-offer] Found", amuletContracts.length, "Amulet contracts");

      // Extract contract IDs and blobs from holdings
      const amuletCids: string[] = [];
      const amuletDisclosed: Array<{
        contractId: string;
        templateId: string;
        createdEventBlob: string;
        synchronizerId: string;
      }> = [];

      for (const contract of amuletContracts) {
        const createdEvent = contract.contractEntry?.JsActiveContract?.createdEvent;
        const cid = createdEvent?.contractId || createdEvent?.contract_id;
        const tid = createdEvent?.templateId || createdEvent?.template_id;
        const blob = createdEvent?.createdEventBlob || createdEvent?.created_event_blob;

        if (cid && tid && blob) {
          amuletCids.push(cid);
          // We'll add synchronizerId after fetching context
          amuletDisclosed.push({
            contractId: cid,
            templateId: tid,
            createdEventBlob: blob,
            synchronizerId: "", // Will be filled later
          });
        }
      }

      if (amuletCids.length === 0) {
        return { success: false, error: "Could not extract contract IDs from Amulet holdings" };
      }

      console.log("[accept-credential-offer] Prepared", amuletCids.length, "Amulet contracts for deposit");

      // Fetch app transfer context from Scan API
      console.log("[accept-credential-offer] Fetching app transfer context...");
      const context = await fetchAppTransferContext();
      console.log("[accept-credential-offer] Got app transfer context:", {
        amuletRulesContractId: context.amuletRulesContractId.substring(0, 20) + "...",
        openRoundContractId: context.openRoundContractId.substring(0, 20) + "...",
        featuredAppRightContractId: context.featuredAppRightContractId?.substring(0, 20) + "..." || "null",
        synchronizerId: context.synchronizerId,
      });

      // Update amulet disclosed contracts with synchronizerId
      for (const disclosed of amuletDisclosed) {
        disclosed.synchronizerId = context.synchronizerId;
      }

      // Build appTransferContext - plain contract IDs as strings
      const appTransferContext = {
        amuletRules: context.amuletRulesContractId,
        openMiningRound: context.openRoundContractId,
        featuredAppRight: context.featuredAppRightContractId,
      };

      // Build disclosed contracts array
      const disclosedContracts = [
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
        // User's Amulet contracts
        ...amuletDisclosed,
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

      command = {
        commands: [{
          ExerciseCommand: {
            templateId,
            contractId: userServiceCid,
            choice: "UserService_AcceptPaidCredentialOffer",
            choiceArgument: {
              credentialOfferCid,
              depositAmulets: amuletCids,
              appTransferContext,
            },
          },
        }],
        disclosedContracts,
      };
    } else {
      // For free credentials, just exercise the accept choice
      command = {
        commands: [{
          ExerciseCommand: {
            templateId,
            contractId: userServiceCid,
            choice: "UserService_AcceptFreeCredentialOffer",
            choiceArgument: {
              credentialOfferCid,
            },
          },
        }],
        disclosedContracts: [],
      };
    }

    console.log("[accept-credential-offer] Submitting command:", JSON.stringify(command, null, 2));

    const result = await provider.submitTransaction(command);
    console.log("[accept-credential-offer] Submit result:", result);

    return {
      success: true,
      submissionId: result?.submission_id,
      commandId: result?.command_id,
    };
  } catch (error) {
    console.error("[accept-credential-offer] Error:", error);
    return {
      success: false,
      error: error instanceof Error ? error.message : "Accept failed",
    };
  }
}
