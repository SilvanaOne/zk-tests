import { NextRequest, NextResponse } from "next/server";

export type LoopNetwork = "devnet" | "testnet" | "mainnet";

// Scan API is accessed via the validator's scan proxy
function getScanBaseUrl(_network: LoopNetwork): string {
  return process.env.SCAN_API_BASE_URL || "http://localhost:8080";
}

export interface PreapprovalContract {
  contractId: string;
  templateId: string;
  provider: string;
  receiver: string;
  expiresAt?: string;
  createdAt?: string;
}

export interface PreapprovalsResponse {
  accepted: PreapprovalContract | null;
}

export async function GET(request: NextRequest) {
  const { searchParams } = new URL(request.url);
  const network = (searchParams.get("network") || "devnet") as LoopNetwork;
  const partyId = searchParams.get("partyId");

  if (!partyId) {
    return NextResponse.json(
      { error: "partyId is required" },
      { status: 400 }
    );
  }

  const baseUrl = getScanBaseUrl(network);

  try {
    console.log("[Scan API] Fetching preapproval contracts for party:", partyId);

    // Fetch accepted TransferPreapproval for this party
    // The Scan API endpoint is: GET /api/scan/v0/transfer-preapprovals/by-party/{party}
    const preapprovalResponse = await fetch(
      `${baseUrl}/api/scan/v0/transfer-preapprovals/by-party/${encodeURIComponent(partyId)}`,
      {
        method: "GET",
        headers: { "Content-Type": "application/json" },
      }
    );

    let accepted: PreapprovalContract | null = null;

    // Handle accepted preapproval (single result from Scan API)
    if (preapprovalResponse.ok) {
      const preapprovalData = await preapprovalResponse.json();
      console.log("[Scan API] TransferPreapproval response:", JSON.stringify(preapprovalData, null, 2));

      if (preapprovalData.transfer_preapproval?.contract) {
        const contract = preapprovalData.transfer_preapproval.contract;
        accepted = {
          contractId: contract.contract_id,
          templateId: contract.template_id,
          provider: contract.payload?.provider || "",
          receiver: contract.payload?.receiver || partyId,
          expiresAt: contract.payload?.expiresAt,
          createdAt: contract.created_at,
        };
      }
    } else {
      console.log("[Scan API] No accepted preapproval found for party (status:", preapprovalResponse.status, ")");
    }

    console.log("[Scan API] Returning preapproval:", { accepted: !!accepted });

    return NextResponse.json({ accepted } as PreapprovalsResponse);
  } catch (error) {
    console.error("[Scan API] Error fetching preapprovals:", error);
    return NextResponse.json(
      { error: "Internal server error while fetching preapprovals" },
      { status: 500 }
    );
  }
}
