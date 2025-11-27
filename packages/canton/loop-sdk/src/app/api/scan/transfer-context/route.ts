import { NextRequest, NextResponse } from "next/server";

export type LoopNetwork = "devnet" | "testnet" | "mainnet";

// Disclosed contract info needed for DAML commands
interface DisclosedContractInfo {
  contractId: string;
  templateId: string;
  createdEventBlob: string;
}

interface TransferContextResponse {
  amuletRulesContractId: string;
  openRoundContractId: string;
  transferPreapprovalContractId: string;
  featuredAppRightContractId: string | null;
  externalPartyAmuletRulesContractId: string;
  dsoParty: string;
  // For disclosed contracts
  amuletRules: DisclosedContractInfo;
  openMiningRound: DisclosedContractInfo;
  transferPreapproval: DisclosedContractInfo;
  featuredAppRight: DisclosedContractInfo | null;
  externalPartyAmuletRules: DisclosedContractInfo;
  synchronizerId: string;
}

// Scan API is accessed via the validator's scan proxy
// Use SCAN_API_BASE_URL environment variable or default to localhost
function getScanBaseUrl(_network: LoopNetwork): string {
  return process.env.SCAN_API_BASE_URL || "http://localhost:8080";
}

export async function GET(request: NextRequest) {
  const { searchParams } = new URL(request.url);
  const network = (searchParams.get("network") || "devnet") as LoopNetwork;
  const receiverParty = searchParams.get("receiverParty");

  if (!receiverParty) {
    return NextResponse.json(
      { error: "receiverParty is required" },
      { status: 400 }
    );
  }

  const baseUrl = getScanBaseUrl(network);

  try {
    console.log("[Scan API] Fetching transfer context for receiver:", receiverParty, "network:", network);
    console.log("[Scan API] Using base URL:", baseUrl);

    // Fetch all context data in parallel
    const [dsoResponse, roundsResponse, preapprovalResponse, featuredAppsResponse, externalPartyResponse] = await Promise.all([
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
      // 3. GET /api/scan/v0/transfer-preapprovals/by-party/{party} - Get receiver's TransferPreapproval
      fetch(`${baseUrl}/api/scan/v0/transfer-preapprovals/by-party/${encodeURIComponent(receiverParty)}`, {
        method: "GET",
        headers: { "Content-Type": "application/json" },
      }),
      // 4. GET /api/scan/v0/featured-apps - Get FeaturedAppRight contracts
      fetch(`${baseUrl}/api/scan/v0/featured-apps`, {
        method: "GET",
        headers: { "Content-Type": "application/json" },
      }),
      // 5. POST /api/scan/v0/external-party-amulet-rules - Get ExternalPartyAmuletRules (TransferFactory)
      fetch(`${baseUrl}/api/scan/v0/external-party-amulet-rules`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          cached_external_party_amulet_rules_contract_id: "",
          cached_external_party_amulet_rules_domain_id: "",
        }),
      }),
    ]);

    if (!dsoResponse.ok) {
      const error = await dsoResponse.text();
      console.error("[Scan API] DSO fetch failed:", error);
      return NextResponse.json(
        { error: `Failed to fetch DSO data: ${dsoResponse.status}` },
        { status: 500 }
      );
    }

    if (!roundsResponse.ok) {
      const error = await roundsResponse.text();
      console.error("[Scan API] Rounds fetch failed:", error);
      return NextResponse.json(
        { error: `Failed to fetch mining rounds: ${roundsResponse.status}` },
        { status: 500 }
      );
    }

    if (!preapprovalResponse.ok) {
      const error = await preapprovalResponse.text();
      console.error("[Scan API] TransferPreapproval fetch failed:", error);
      return NextResponse.json(
        { error: `Failed to fetch transfer preapproval for receiver: ${preapprovalResponse.status}. Receiver may not have a preapproval contract.` },
        { status: 404 }
      );
    }

    if (!externalPartyResponse.ok) {
      const error = await externalPartyResponse.text();
      console.error("[Scan API] ExternalPartyAmuletRules fetch failed:", error);
      return NextResponse.json(
        { error: `Failed to fetch ExternalPartyAmuletRules: ${externalPartyResponse.status}` },
        { status: 500 }
      );
    }

    const dsoData = await dsoResponse.json();
    const roundsData = await roundsResponse.json();
    const preapprovalData = await preapprovalResponse.json();
    // FeaturedApps is optional - don't fail if it's not available
    const featuredAppsData = featuredAppsResponse.ok ? await featuredAppsResponse.json() : { featured_apps: [] };
    const externalPartyData = await externalPartyResponse.json();

    console.log("[Scan API] DSO response received, has amulet_rules:", !!dsoData.amulet_rules);
    console.log("[Scan API] Rounds response received, open_mining_rounds count:", Object.keys(roundsData.open_mining_rounds || {}).length);
    console.log("[Scan API] Preapproval response received, has transfer_preapproval:", !!preapprovalData.transfer_preapproval);
    console.log("[Scan API] FeaturedApps response received, count:", (featuredAppsData.featured_apps || []).length);
    console.log("[Scan API] ExternalPartyAmuletRules response received, has update:", !!externalPartyData.external_party_amulet_rules_update);

    // Extract contract IDs and blobs from responses
    // DSO response: { amulet_rules: { contract: { contract_id: "...", template_id: "...", created_event_blob: "..." } }, dso_rules: { ... }, sv_node_states: [...] }
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

    // Rounds response: { open_mining_rounds: { [contractId]: { contract: { ... } } }, issuing_mining_rounds: {...} }
    // The response is a map, not an array - convert to array and sort by opensAt (earliest first)
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

    // Get synchronizerId from the rounds response (it's at the state level)
    const synchronizerId = earliestRound?.roundData?.state?.synchronizerId ||
      earliestRound?.roundData?.synchronizer_id ||
      `global-domain::${dsoParty.split("::")[1]}`; // Fallback: derive from DSO party

    // Preapproval response: { transfer_preapproval: { contract: { contract_id: "...", template_id: "...", created_event_blob: "..." } } }
    const transferPreapprovalContract = preapprovalData.transfer_preapproval?.contract;
    const transferPreapprovalContractId = transferPreapprovalContract?.contract_id;
    const transferPreapprovalTemplateId = transferPreapprovalContract?.template_id;
    const transferPreapprovalBlob = transferPreapprovalContract?.created_event_blob;

    if (!transferPreapprovalContractId || !transferPreapprovalTemplateId || !transferPreapprovalBlob) {
      return NextResponse.json(
        { error: "TransferPreapproval contract data incomplete for receiver" },
        { status: 404 }
      );
    }

    // Get the provider from the TransferPreapproval contract - this is who we need to find FeaturedAppRight for
    const transferPreapprovalProvider = transferPreapprovalContract?.payload?.provider;
    console.log("[Scan API] TransferPreapproval provider:", transferPreapprovalProvider);

    // Extract FeaturedAppRight - find one matching the TransferPreapproval's provider
    // The featured_apps array contains objects with contract_id, template_id, created_event_blob, and payload
    const featuredApps = featuredAppsData.featured_apps || [];
    let featuredAppRight: DisclosedContractInfo | null = null;
    let featuredAppRightContractId: string | null = null;

    if (featuredApps.length > 0) {
      // Find FeaturedAppRight matching the TransferPreapproval's provider
      let matchedApp = featuredApps.find((app: any) =>
        app.payload?.provider === transferPreapprovalProvider
      );

      // Fallback to first available if no match (shouldn't happen if provider has FeaturedAppRight)
      if (!matchedApp && transferPreapprovalProvider) {
        console.log("[Scan API] No FeaturedAppRight found for provider:", transferPreapprovalProvider, "- using first available");
        matchedApp = featuredApps[0];
      } else if (!matchedApp) {
        console.log("[Scan API] No TransferPreapproval provider found, using first FeaturedAppRight");
        matchedApp = featuredApps[0];
      }

      featuredAppRightContractId = matchedApp.contract_id;
      const featuredAppTemplateId = matchedApp.template_id;
      const featuredAppBlob = matchedApp.created_event_blob;

      if (featuredAppRightContractId && featuredAppTemplateId && featuredAppBlob) {
        featuredAppRight = {
          contractId: featuredAppRightContractId,
          templateId: featuredAppTemplateId,
          createdEventBlob: featuredAppBlob,
        };
        console.log("[Scan API] Found FeaturedAppRight for provider:", matchedApp.payload?.provider);
        console.log("[Scan API] FeaturedAppRight contractId:", featuredAppRightContractId.substring(0, 20) + "...");
      }
    }

    // Extract ExternalPartyAmuletRules - this is the TransferFactory contract
    // Response: { external_party_amulet_rules_update: { contract: { contract_id, template_id, created_event_blob } } }
    const externalPartyContract = externalPartyData.external_party_amulet_rules_update?.contract;
    const externalPartyContractId = externalPartyContract?.contract_id;
    const externalPartyTemplateId = externalPartyContract?.template_id;
    const externalPartyBlob = externalPartyContract?.created_event_blob;

    if (!externalPartyContractId || !externalPartyTemplateId || !externalPartyBlob) {
      return NextResponse.json(
        { error: "ExternalPartyAmuletRules contract data incomplete" },
        { status: 500 }
      );
    }

    const externalPartyAmuletRules: DisclosedContractInfo = {
      contractId: externalPartyContractId,
      templateId: externalPartyTemplateId,
      createdEventBlob: externalPartyBlob,
    };

    console.log("[Scan API] ExternalPartyAmuletRules contractId:", externalPartyContractId.substring(0, 20) + "...");
    console.log("[Scan API] ExternalPartyAmuletRules templateId:", externalPartyTemplateId);

    const result: TransferContextResponse = {
      amuletRulesContractId,
      openRoundContractId,
      transferPreapprovalContractId,
      featuredAppRightContractId,
      externalPartyAmuletRulesContractId: externalPartyContractId,
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
      transferPreapproval: {
        contractId: transferPreapprovalContractId,
        templateId: transferPreapprovalTemplateId,
        createdEventBlob: transferPreapprovalBlob,
      },
      featuredAppRight,
      externalPartyAmuletRules,
      synchronizerId,
    };

    console.log("[Scan API] Returning transfer context:");
    console.log("  - amuletRulesContractId:", amuletRulesContractId.substring(0, 20) + "...");
    console.log("  - amuletRulesTemplateId:", amuletRulesTemplateId);
    console.log("  - amuletRulesBlob length:", amuletRulesBlob.length);
    console.log("  - openRoundContractId:", openRoundContractId.substring(0, 20) + "...");
    console.log("  - openRoundTemplateId:", openRoundTemplateId);
    console.log("  - openRoundBlob length:", openRoundBlob.length);
    console.log("  - transferPreapprovalContractId:", transferPreapprovalContractId.substring(0, 20) + "...");
    console.log("  - transferPreapprovalTemplateId:", transferPreapprovalTemplateId);
    console.log("  - transferPreapprovalBlob length:", transferPreapprovalBlob.length);
    console.log("  - featuredAppRightContractId:", featuredAppRightContractId ? featuredAppRightContractId.substring(0, 20) + "..." : "null");
    if (featuredAppRight) {
      console.log("  - featuredAppRight.templateId:", featuredAppRight.templateId);
      console.log("  - featuredAppRight.blob length:", featuredAppRight.createdEventBlob.length);
    }
    console.log("  - externalPartyAmuletRulesContractId:", externalPartyContractId.substring(0, 20) + "...");
    console.log("  - externalPartyAmuletRules.templateId:", externalPartyTemplateId);
    console.log("  - externalPartyAmuletRules.blob length:", externalPartyBlob.length);
    console.log("  - synchronizerId:", synchronizerId);
    console.log("  - dsoParty:", dsoParty);

    return NextResponse.json(result);
  } catch (error) {
    console.error("[Scan API] Error fetching transfer context:", error);
    return NextResponse.json(
      { error: "Internal server error while fetching transfer context" },
      { status: 500 }
    );
  }
}
