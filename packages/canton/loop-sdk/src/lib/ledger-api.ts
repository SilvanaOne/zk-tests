"use server";

import type { LedgerHolding, LedgerActiveContract } from "./ledger-api-types";

const LEDGER_API_BASE_URL = process.env.LEDGER_API_BASE_URL || "http://116.203.0.152:7575";
const JWT_TOKEN = process.env.JWT_SETTLEMENT_OPERATOR;

interface ActiveContractResponse {
  contractEntry: {
    JsActiveContract: {
      createdEvent: {
        contractId: string;
        templateId: string;
        createArgument: any;
        createdEventBlob?: string;
      };
    };
  };
}

/**
 * Get the current ledger end offset
 */
export async function getLedgerEnd(): Promise<string> {
  const url = `${LEDGER_API_BASE_URL}/api/json-api/v2/state/ledger-end`;

  const response = await fetch(url, {
    method: "GET",
    headers: {
      "Authorization": `Bearer ${JWT_TOKEN}`,
      "Content-Type": "application/json",
    },
  });

  if (!response.ok) {
    const text = await response.text();
    console.error("[ledger-api] getLedgerEnd failed:", response.status, text);
    throw new Error(`Failed to get ledger end: ${response.status}`);
  }

  const data = await response.json();
  return data.offset;
}

/**
 * Query active contracts for a party at a specific offset
 */
export async function queryActiveContracts(
  partyId: string,
  offset: string
): Promise<ActiveContractResponse[]> {
  const url = `${LEDGER_API_BASE_URL}/api/json-api/v2/state/active-contracts`;

  const requestBody = {
    filter: {
      filtersByParty: {
        [partyId]: {
          cumulative: [],
        },
      },
    },
    verbose: true,
    activeAtOffset: offset,
  };

  console.log("[ledger-api] queryActiveContracts request:", JSON.stringify(requestBody, null, 2));

  const response = await fetch(url, {
    method: "POST",
    headers: {
      "Authorization": `Bearer ${JWT_TOKEN}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify(requestBody),
  });

  if (!response.ok) {
    const text = await response.text();
    console.error("[ledger-api] queryActiveContracts failed:", response.status, text);
    throw new Error(`Failed to query active contracts: ${response.status}`);
  }

  const data = await response.json();
  return data || [];
}

/**
 * Get all active contracts for a party
 */
export async function getActiveContractsFromLedger(
  partyId: string
): Promise<LedgerActiveContract[]> {
  try {
    const offset = await getLedgerEnd();
    console.log("[ledger-api] Got ledger end offset:", offset);

    const contracts = await queryActiveContracts(partyId, offset);
    console.log("[ledger-api] Got", contracts.length, "contracts");

    return contracts.map((c) => ({
      contractId: c.contractEntry.JsActiveContract.createdEvent.contractId,
      templateId: c.contractEntry.JsActiveContract.createdEvent.templateId,
      createArgument: c.contractEntry.JsActiveContract.createdEvent.createArgument,
      createdEventBlob: c.contractEntry.JsActiveContract.createdEvent.createdEventBlob,
    }));
  } catch (error) {
    console.error("[ledger-api] getActiveContractsFromLedger error:", error);
    return [];
  }
}

/**
 * Get holdings (filtered by Holding:Holding and Splice.Amulet templates)
 */
export async function getHoldingsFromLedger(
  partyId: string
): Promise<LedgerHolding[]> {
  try {
    const contracts = await getActiveContractsFromLedger(partyId);
    const holdings: LedgerHolding[] = [];

    for (const contract of contracts) {
      const { templateId, createArgument, contractId } = contract;

      // Check if this is a Holding contract
      if (templateId.includes("Holding:Holding")) {
        const tokenId = createArgument?.instrument?.id || "Unknown";
        const amount = createArgument?.amount || "0";
        const lock = createArgument?.lock;

        const isLocked = lock && lock.lockers && Object.keys(lock.lockers?.map || {}).length > 0;

        holdings.push({
          contractId,
          templateId,
          tokenId,
          amount: String(amount),
          isLocked: !!isLocked,
          lockInfo: isLocked ? {
            holders: lock.lockers?.map ? Object.keys(lock.lockers.map) : [],
            expiresAt: lock.lockUntil,
            context: lock.context,
          } : undefined,
        });
      }
      // Check if this is a Canton Coin (Amulet) contract
      else if (templateId.includes("Splice.Amulet:Amulet")) {
        // Canton Coin amount is nested differently
        const amount = createArgument?.amulet?.amount?.initialAmount
                    || createArgument?.amount?.initialAmount
                    || "0";
        const lock = createArgument?.lock;

        const isLocked = lock && lock.holders && lock.holders.length > 0;

        holdings.push({
          contractId,
          templateId,
          tokenId: "CC", // Canton Coin
          amount: String(amount),
          isLocked: !!isLocked,
          lockInfo: isLocked ? {
            holders: lock.holders,
            expiresAt: lock.expiresAt,
            context: lock.optContext,
          } : undefined,
        });
      }
    }

    console.log("[ledger-api] Found", holdings.length, "holdings");
    return holdings;
  } catch (error) {
    console.error("[ledger-api] getHoldingsFromLedger error:", error);
    return [];
  }
}

/**
 * Get preapproval contracts (TransferPreapproval)
 */
export async function getPreapprovalsFromLedger(
  partyId: string
): Promise<LedgerActiveContract[]> {
  try {
    const contracts = await getActiveContractsFromLedger(partyId);
    return contracts.filter((c) =>
      c.templateId.includes("TransferPreapproval:TransferPreapproval") ||
      c.templateId.includes("TransferPreapprovalProposal")
    );
  } catch (error) {
    console.error("[ledger-api] getPreapprovalsFromLedger error:", error);
    return [];
  }
}

/**
 * Get Amulet (Canton Coin) contract IDs for a party
 * Returns array of contract IDs that can be used as inputHoldingCids for transfers
 */
export async function getAmuletContractsFromLedger(
  partyId: string
): Promise<LedgerActiveContract[]> {
  try {
    const contracts = await getActiveContractsFromLedger(partyId);
    // Filter for unlocked Amulet contracts only (not LockedAmulet)
    // Template ID contains "Splice.Amulet:Amulet" but not "LockedAmulet"
    return contracts.filter((c) => {
      const isAmulet = c.templateId.includes("Splice.Amulet:Amulet");
      const isLocked = c.templateId.includes("LockedAmulet");
      return isAmulet && !isLocked;
    });
  } catch (error) {
    console.error("[ledger-api] getAmuletContractsFromLedger error:", error);
    return [];
  }
}
