"use server";

/**
 * Server actions for Canton interactive submission flow.
 *
 * This replaces the API routes with "use server" functions for better
 * integration with Next.js App Router.
 *
 * Flow:
 * 1. Prepare - Get the transaction hash to sign
 * 2. Sign - Client signs the hash with their wallet (client-side)
 * 3. Execute - Submit the signed transaction
 */

// ============================================================================
// Types
// ============================================================================

interface PrepareParams {
  externalPartyId: string;
  commands: any[];
  disclosedContracts?: any[];
}

interface PrepareResult {
  success: boolean;
  preparedTransaction?: string;
  preparedTransactionHash?: string;
  hashingSchemeVersion?: string;
  error?: string;
}

interface ExecuteParams {
  preparedTransaction: string;
  preparedTransactionHash: string;
  hashingSchemeVersion: string;
  externalPartyId: string;
  signatureBase64: string;
}

interface ExecuteResult {
  success: boolean;
  updateId?: string;
  submissionId?: string;
  error?: string;
}

// ============================================================================
// Helper Functions
// ============================================================================

function extractUserIdFromJwt(jwt: string): string | null {
  try {
    const parts = jwt.split(".");
    if (parts.length !== 3) return null;

    const payload = parts[1].replace(/-/g, "+").replace(/_/g, "/");
    const decoded = atob(payload);
    const claims = JSON.parse(decoded);

    return claims.sub || null;
  } catch (error) {
    console.error("[interactive-submission] Failed to extract userId from JWT:", error);
    return null;
  }
}

function getFingerprintFromParty(partyId: string): string | null {
  const parts = partyId.split("::");
  if (parts.length !== 2) return null;
  return parts[1];
}

// ============================================================================
// Server Actions
// ============================================================================

/**
 * Prepare a Canton transaction for interactive submission.
 * Returns the transaction hash that needs to be signed by the external party.
 */
export async function prepareInteractiveSubmission(params: PrepareParams): Promise<PrepareResult> {
  const { externalPartyId, commands, disclosedContracts = [] } = params;

  console.log("[prepareInteractiveSubmission] Starting...");
  console.log("[prepareInteractiveSubmission] externalPartyId:", externalPartyId);
  console.log("[prepareInteractiveSubmission] commands:", JSON.stringify(commands, null, 2));

  // Validate inputs
  if (!externalPartyId) {
    console.error("[prepareInteractiveSubmission] Missing externalPartyId");
    return { success: false, error: "externalPartyId is required" };
  }

  if (!commands || commands.length === 0) {
    console.error("[prepareInteractiveSubmission] Missing commands");
    return { success: false, error: "commands array is required and must not be empty" };
  }

  // Get environment variables
  const ledgerApiBaseUrl = process.env.LEDGER_API_BASE_URL;
  const jwt = process.env.JWT_SETTLEMENT_OPERATOR;
  const synchronizerId = process.env.SYNCHRONIZER_ID;

  console.log("[prepareInteractiveSubmission] Environment check:", {
    hasLedgerApiBaseUrl: !!ledgerApiBaseUrl,
    ledgerApiBaseUrl: ledgerApiBaseUrl?.substring(0, 30) + "...",
    hasJwt: !!jwt,
    jwtLength: jwt?.length,
    hasSynchronizerId: !!synchronizerId,
    synchronizerId: synchronizerId?.substring(0, 30) + "...",
  });

  if (!ledgerApiBaseUrl || !jwt || !synchronizerId) {
    console.error("[prepareInteractiveSubmission] Missing environment variables");
    return { success: false, error: "Server configuration error: missing environment variables" };
  }

  // Extract userId from JWT
  const userId = extractUserIdFromJwt(jwt);
  if (!userId) {
    console.error("[prepareInteractiveSubmission] Failed to extract userId from JWT");
    return { success: false, error: "Failed to extract userId from JWT" };
  }

  console.log("[prepareInteractiveSubmission] Extracted userId:", userId);

  // Build the prepare submission request
  const commandId = `phantom-preapproval-${Date.now()}`;
  const preparePayload = {
    userId,
    commandId,
    actAs: [externalPartyId],
    readAs: [externalPartyId],
    synchronizerId,
    packageIdSelectionPreference: [],
    verboseHashing: false,
    commands,
    disclosedContracts,
  };

  // Canton ledger API requires /api/json-api/v2 prefix
  const url = `${ledgerApiBaseUrl}/api/json-api/v2/interactive-submission/prepare`;
  console.log("[prepareInteractiveSubmission] Calling Canton prepare endpoint:", url);
  console.log("[prepareInteractiveSubmission] Payload:", JSON.stringify(preparePayload, null, 2));

  try {
    const response = await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "Authorization": `Bearer ${jwt}`,
      },
      body: JSON.stringify(preparePayload),
    });

    const responseText = await response.text();
    console.log("[prepareInteractiveSubmission] Response status:", response.status);
    console.log("[prepareInteractiveSubmission] Response body:", responseText.substring(0, 500));

    if (!response.ok) {
      console.error("[prepareInteractiveSubmission] Canton prepare failed:", {
        status: response.status,
        body: responseText,
      });

      let errorMessage = `Prepare failed: ${response.status}`;
      if (response.status === 403) {
        errorMessage = "Authorization failed. The service account may not have CanExecuteAsAnyParty rights for this external party.";
      }

      return { success: false, error: errorMessage };
    }

    const responseData = JSON.parse(responseText);
    console.log("[prepareInteractiveSubmission] Canton prepare succeeded:", {
      hasTransaction: !!responseData.preparedTransaction,
      transactionLength: responseData.preparedTransaction?.length,
      hasHash: !!responseData.preparedTransactionHash,
      hashLength: responseData.preparedTransactionHash?.length,
      hashingScheme: responseData.hashingSchemeVersion,
    });

    return {
      success: true,
      preparedTransaction: responseData.preparedTransaction,
      preparedTransactionHash: responseData.preparedTransactionHash,
      hashingSchemeVersion: responseData.hashingSchemeVersion || "HASHING_SCHEME_VERSION_V2",
    };
  } catch (error: any) {
    console.error("[prepareInteractiveSubmission] Error:", error);
    return { success: false, error: error.message || "Failed to prepare transaction" };
  }
}

/**
 * Execute a signed Canton transaction.
 * Submits the prepared transaction with the external party's signature.
 */
export async function executeInteractiveSubmission(params: ExecuteParams): Promise<ExecuteResult> {
  const {
    preparedTransaction,
    preparedTransactionHash,
    hashingSchemeVersion,
    externalPartyId,
    signatureBase64,
  } = params;

  console.log("[executeInteractiveSubmission] Starting...");
  console.log("[executeInteractiveSubmission] externalPartyId:", externalPartyId);
  console.log("[executeInteractiveSubmission] signatureBase64 length:", signatureBase64?.length);
  console.log("[executeInteractiveSubmission] preparedTransactionHash length:", preparedTransactionHash?.length);

  // Validate required fields
  if (!preparedTransaction || !externalPartyId || !signatureBase64) {
    console.error("[executeInteractiveSubmission] Missing required fields");
    return {
      success: false,
      error: "Missing required fields: preparedTransaction, externalPartyId, signatureBase64",
    };
  }

  // Get environment variables
  const ledgerApiBaseUrl = process.env.LEDGER_API_BASE_URL;
  const jwt = process.env.JWT_SETTLEMENT_OPERATOR;

  console.log("[executeInteractiveSubmission] Environment check:", {
    hasLedgerApiBaseUrl: !!ledgerApiBaseUrl,
    hasJwt: !!jwt,
  });

  if (!ledgerApiBaseUrl || !jwt) {
    console.error("[executeInteractiveSubmission] Missing required environment variables");
    return { success: false, error: "Server configuration error" };
  }

  // Extract userId from JWT
  const userId = extractUserIdFromJwt(jwt);
  if (!userId) {
    console.error("[executeInteractiveSubmission] Failed to extract userId from JWT");
    return { success: false, error: "Failed to extract userId from JWT" };
  }

  // Extract fingerprint from party ID for signedBy field
  const fingerprint = getFingerprintFromParty(externalPartyId);
  if (!fingerprint) {
    console.error("[executeInteractiveSubmission] Invalid party ID format:", externalPartyId);
    return { success: false, error: "Invalid party ID format" };
  }

  console.log("[executeInteractiveSubmission] Extracted fingerprint:", fingerprint.substring(0, 20) + "...");

  // Generate submission ID
  const submissionId = `submit-phantom-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;

  // Build the execute submission request
  const executePayload = {
    preparedTransaction,
    partySignatures: {
      signatures: [{
        party: externalPartyId,
        signatures: [{
          format: "SIGNATURE_FORMAT_CONCAT",
          signature: signatureBase64,
          signedBy: fingerprint,
          signingAlgorithmSpec: "SIGNING_ALGORITHM_SPEC_ED25519",
        }],
      }],
    },
    submissionId,
    userId,
    hashingSchemeVersion: hashingSchemeVersion || "HASHING_SCHEME_VERSION_V2",
    deduplicationPeriod: {
      DeduplicationDuration: {
        value: "PT60S",
      },
    },
  };

  // Canton ledger API requires /api/json-api/v2 prefix
  const url = `${ledgerApiBaseUrl}/api/json-api/v2/interactive-submission/execute`;
  console.log("[executeInteractiveSubmission] Calling Canton execute endpoint:", url);
  console.log("[executeInteractiveSubmission] submissionId:", submissionId);

  try {
    const response = await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "Authorization": `Bearer ${jwt}`,
      },
      body: JSON.stringify(executePayload),
    });

    const responseText = await response.text();
    console.log("[executeInteractiveSubmission] Response status:", response.status);
    console.log("[executeInteractiveSubmission] Response body:", responseText.substring(0, 500));

    let responseData: any = {};
    try {
      responseData = JSON.parse(responseText);
    } catch {
      // Response might not be JSON
    }

    if (!response.ok) {
      console.error("[executeInteractiveSubmission] Canton execute failed:", {
        status: response.status,
        body: responseText,
      });

      const errorMessage = responseData.cause ||
        responseData.error ||
        responseData.errors ||
        `Execute failed: ${response.status}`;

      return { success: false, error: errorMessage };
    }

    console.log("[executeInteractiveSubmission] Canton execute succeeded:", {
      updateId: responseData.updateId,
      submissionId,
    });

    return {
      success: true,
      updateId: responseData.updateId || submissionId,
      submissionId,
    };
  } catch (error: any) {
    console.error("[executeInteractiveSubmission] Error:", error);
    return { success: false, error: error.message || "Internal server error" };
  }
}
