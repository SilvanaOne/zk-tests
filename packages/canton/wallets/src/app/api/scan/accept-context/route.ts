import { NextRequest, NextResponse } from "next/server";

export type LoopNetwork = "devnet" | "testnet" | "mainnet";

// Disclosed contract info needed for DAML commands
interface DisclosedContractInfo {
  contractId: string;
  templateId: string;
  createdEventBlob: string;
}

interface AcceptContextResponse {
  amuletRulesContractId: string;
  openRoundContractId: string;
  featuredAppRightContractId: string | null;
  dsoParty: string;
  // For disclosed contracts
  amuletRules: DisclosedContractInfo;
  openMiningRound: DisclosedContractInfo;
  featuredAppRight: DisclosedContractInfo | null;
  synchronizerId: string;
}

// Scan API is accessed via the validator's scan proxy
function getScanBaseUrl(_network: LoopNetwork): string {
  return process.env.SCAN_API_BASE_URL || "http://localhost:8080";
}

export async function GET(request: NextRequest) {
  const { searchParams } = new URL(request.url);
  const network = (searchParams.get("network") || "devnet") as LoopNetwork;
  // Use client-provided providerHint, or fall back to server-side PARTY_SETTLEMENT_OPERATOR
  const providerHint = searchParams.get("providerHint") || process.env.PARTY_SETTLEMENT_OPERATOR || null;

  const baseUrl = getScanBaseUrl(network);

  try {
    console.log("[Accept Context API] Fetching accept context, network:", network);
    console.log("[Accept Context API] Using base URL:", baseUrl);

    // Fetch all context data in parallel
    const [dsoResponse, roundsResponse, featuredAppsResponse] = await Promise.all([
      // 1. GET /api/scan/v0/dso - Get AmuletRules and DSO party
      fetch(`${baseUrl}/api/scan/v0/dso`, {
        method: "GET",
        headers: { "Content-Type": "application/json" },
      }),
      // 2. POST /api/scan/v0/open-and-issuing-mining-rounds - Get OpenMiningRound
      fetch(`${baseUrl}/api/scan/v0/open-and-issuing-mining-rounds`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          cached_open_mining_round_contract_ids: [],
          cached_issuing_round_contract_ids: [],
        }),
      }),
      // 3. GET /api/scan/v0/featured-apps - Get FeaturedAppRight contracts
      fetch(`${baseUrl}/api/scan/v0/featured-apps`, {
        method: "GET",
        headers: { "Content-Type": "application/json" },
      }),
    ]);

    if (!dsoResponse.ok) {
      const error = await dsoResponse.text();
      console.error("[Accept Context API] DSO fetch failed:", error);
      return NextResponse.json(
        { error: `Failed to fetch DSO data: ${dsoResponse.status}` },
        { status: 500 }
      );
    }

    if (!roundsResponse.ok) {
      const error = await roundsResponse.text();
      console.error("[Accept Context API] Rounds fetch failed:", error);
      return NextResponse.json(
        { error: `Failed to fetch mining rounds: ${roundsResponse.status}` },
        { status: 500 }
      );
    }

    const dsoData = await dsoResponse.json();
    const roundsData = await roundsResponse.json();
    // FeaturedApps is optional
    const featuredAppsData = featuredAppsResponse.ok ? await featuredAppsResponse.json() : { featured_apps: [] };

    console.log("[Accept Context API] DSO response received, has amulet_rules:", !!dsoData.amulet_rules);
    console.log("[Accept Context API] Rounds response received, open_mining_rounds count:", Object.keys(roundsData.open_mining_rounds || {}).length);
    console.log("[Accept Context API] FeaturedApps response received, count:", (featuredAppsData.featured_apps || []).length);

    // Extract AmuletRules contract
    const amuletRulesContract = dsoData.amulet_rules?.contract;
    const amuletRulesContractId = amuletRulesContract?.contract_id;
    const amuletRulesTemplateId = amuletRulesContract?.template_id;
    const amuletRulesBlob = amuletRulesContract?.created_event_blob;

    if (!amuletRulesContractId || !amuletRulesTemplateId || !amuletRulesBlob) {
      return NextResponse.json(
        { error: "AmuletRules contract data incomplete in DSO response" },
        { status: 500 }
      );
    }

    // Extract DSO party from the amulet_rules contract payload
    const dsoParty = amuletRulesContract?.payload?.dso;
    if (!dsoParty) {
      return NextResponse.json(
        { error: "DSO party not found in AmuletRules payload" },
        { status: 500 }
      );
    }

    // Get OpenMiningRound - sort by opensAt and take the earliest
    const openRoundsMap = roundsData.open_mining_rounds;
    if (!openRoundsMap || Object.keys(openRoundsMap).length === 0) {
      return NextResponse.json(
        { error: "No open mining rounds found" },
        { status: 500 }
      );
    }

    // Convert map to array and sort by opensAt (earliest first)
    const sortedRounds = Object.entries(openRoundsMap)
      .map(([contractId, roundData]: [string, any]) => ({
        contractId,
        roundData,
        opensAt: roundData?.contract?.payload?.opensAt || "",
      }))
      .sort((a, b) => a.opensAt.localeCompare(b.opensAt));

    const earliestRound = sortedRounds[0];
    const openRoundContract = earliestRound?.roundData?.contract;
    const openRoundContractId = openRoundContract?.contract_id;
    const openRoundTemplateId = openRoundContract?.template_id;
    const openRoundBlob = openRoundContract?.created_event_blob;

    if (!openRoundContractId || !openRoundTemplateId || !openRoundBlob) {
      return NextResponse.json(
        { error: "OpenMiningRound contract data incomplete" },
        { status: 500 }
      );
    }

    // Get synchronizerId from env or derive from DSO party
    const synchronizerId = process.env.SYNCHRONIZER_ID ||
      earliestRound?.roundData?.state?.synchronizerId ||
      `global-domain::${dsoParty.split("::")[1]}`;

    // Extract FeaturedAppRight - find one matching provider hint if provided
    const featuredApps = featuredAppsData.featured_apps || [];
    let featuredAppRight: DisclosedContractInfo | null = null;
    let featuredAppRightContractId: string | null = null;

    if (featuredApps.length > 0) {
      let matchedApp = null;

      // Try to match provider hint with exact match
      if (providerHint) {
        console.log("[Accept Context API] Looking for FeaturedAppRight with provider:", providerHint);
        matchedApp = featuredApps.find((app: any) =>
          app.payload?.provider === providerHint
        );

        if (!matchedApp) {
          // If providerHint was specified but not found, return error - don't fallback to wrong provider
          console.error("[Accept Context API] No FeaturedAppRight found for provider:", providerHint);
          return NextResponse.json(
            { error: `No FeaturedAppRight found for provider: ${providerHint}` },
            { status: 404 }
          );
        }
      } else {
        // No provider hint - use first available (for backwards compatibility)
        matchedApp = featuredApps[0];
      }

      if (matchedApp) {
        featuredAppRightContractId = matchedApp.contract_id;
        const featuredAppTemplateId = matchedApp.template_id;
        const featuredAppBlob = matchedApp.created_event_blob;

        if (featuredAppRightContractId && featuredAppTemplateId && featuredAppBlob) {
          featuredAppRight = {
            contractId: featuredAppRightContractId,
            templateId: featuredAppTemplateId,
            createdEventBlob: featuredAppBlob,
          };
          console.log("[Accept Context API] Found FeaturedAppRight:", featuredAppRightContractId.substring(0, 20) + "...");
          console.log("[Accept Context API] FeaturedAppRight provider:", matchedApp.payload?.provider);
        }
      }
    }

    const result: AcceptContextResponse = {
      amuletRulesContractId,
      openRoundContractId,
      featuredAppRightContractId,
      dsoParty,
      // Full contract info for disclosed contracts
      amuletRules: {
        contractId: amuletRulesContractId,
        templateId: amuletRulesTemplateId,
        createdEventBlob: amuletRulesBlob,
      },
      openMiningRound: {
        contractId: openRoundContractId,
        templateId: openRoundTemplateId,
        createdEventBlob: openRoundBlob,
      },
      featuredAppRight,
      synchronizerId,
    };

    console.log("[Accept Context API] Returning context:");
    console.log("  - amuletRulesContractId:", amuletRulesContractId.substring(0, 20) + "...");
    console.log("  - openRoundContractId:", openRoundContractId.substring(0, 20) + "...");
    console.log("  - featuredAppRightContractId:", featuredAppRightContractId ? featuredAppRightContractId.substring(0, 20) + "..." : "null");
    console.log("  - synchronizerId:", synchronizerId);
    console.log("  - dsoParty:", dsoParty);

    return NextResponse.json(result);
  } catch (error) {
    console.error("[Accept Context API] Error fetching accept context:", error);
    return NextResponse.json(
      { error: "Internal server error while fetching accept context" },
      { status: 500 }
    );
  }
}
