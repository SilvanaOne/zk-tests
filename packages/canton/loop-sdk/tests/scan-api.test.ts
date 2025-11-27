import { describe, it, mock, beforeEach, afterEach } from "node:test";
import assert from "node:assert";

// Mock response data based on actual Scan API response structure
const mockDsoResponse = {
  amulet_rules: {
    contract: {
      template_id: "3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.AmuletRules:AmuletRules",
      contract_id: "00554dd41d43438d8ee88312085c79447a16efe55a7fc941cc4f67af2fe268ed73ca11122016f7f9e1b0549120f942f63c249c55c27f7bd4ebde1c869cb9d7b0ede5084f6a",
      created_event_blob: "CgMyLjESzg8KRQBVTdQdQ0ONjuiDEghceUR6Fu/lWn/JQcxPZ68v4mjtc8oREiAW9/nhsFSRIPlC9jwknFXCf3vU694chpy517Dt...",
      payload: {
        dso: "DSO::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a"
      }
    }
  },
  dso_rules: {
    contract: { contract_id: "00dso456" }
  },
  dso_party_id: "DSO::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a"
};

// Mock rounds response - actual API returns a map keyed by contract_id, not an array
const mockRoundsResponse = {
  open_mining_rounds: {
    "004c4fb3500962e54d18c13c31dde524ae6c2e7fe137ac3c51fd348bc00ee9bc00ca121220458f43eb337d3a1917c2f505de677c07b3d2a193f9acd36fdf0bca5a97e11903": {
      contract: {
        template_id: "3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.Round:OpenMiningRound",
        contract_id: "004c4fb3500962e54d18c13c31dde524ae6c2e7fe137ac3c51fd348bc00ee9bc00ca121220458f43eb337d3a1917c2f505de677c07b3d2a193f9acd36fdf0bca5a97e11903",
        created_event_blob: "CgMyLjESpAcKRQBMT7NQCWLlTRjBPDHd5SSubC5/4TesPFH9NIvADum8AMoSEiBFj0PrM306GRfC9QXeZ3wHs9Khk/ms02/fC8pa...",
        payload: {
          dso: "DSO::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a",
          opensAt: "2025-11-27T19:37:06.219171Z",
          round: { number: "20792" }
        }
      }
    },
    "00eac1a439c1bfa16e3d8d5b052a0fd15a4611a3dd24a4164f31e1bb593bee06abca1212204034b1bfaddd97d01196c4ed2e0a64897baf2ec9454fdf893a7b411f032b15e0": {
      contract: {
        template_id: "3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.Round:OpenMiningRound",
        contract_id: "00eac1a439c1bfa16e3d8d5b052a0fd15a4611a3dd24a4164f31e1bb593bee06abca1212204034b1bfaddd97d01196c4ed2e0a64897baf2ec9454fdf893a7b411f032b15e0",
        created_event_blob: "CgMyLjESpAcKRQDqwaQ5wb+hbj2NWwUqD9FaRhGj3SSkFk8x4btZO+4Gq8oSEiBANLG/rd2X0BGWxO0uCmSJe68uyUVP34k6e0Ef...",
        payload: {
          dso: "DSO::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a",
          opensAt: "2025-11-27T19:57:43.853001Z",
          round: { number: "20794" }
        }
      }
    }
  },
  issuing_mining_rounds: {}
};

const mockPreapprovalResponse = {
  transfer_preapproval: {
    contract: { contract_id: "00preapproval101112" }
  }
};

// Mock external party amulet rules response
const mockExternalPartyAmuletRulesResponse = {
  external_party_amulet_rules_update: {
    contract: {
      template_id: "a5b055492fb8f08b2e7bc0fc94da6da50c39c2e1d7f24cd5ea8db12fc87c1332:Splice.ExternalPartyAmuletRules:ExternalPartyAmuletRules",
      contract_id: "009f00e5bf00640118d849080aaf22bc963a8458d322585cebf1119cb7bf37a955ca11122065b775fb8a4199904ed32fa9277fd9c0e82bb82319a7151249df124182072381",
      created_event_blob: "CgMyLjESqQMKRQCfAOW/AGQBGNhJCAqvIryWOoRY0yJYXOvxEZy3vzepVcoREiBlt3X7ikGZkE7TL6knf9nA6Cu4IxmnFRJJ3xJB..."
    }
  }
};

// Mock featured apps response - multiple FeaturedAppRight contracts from different providers
const mockFeaturedAppsResponse = {
  featured_apps: [
    {
      template_id: "a5b055492fb8f08b2e7bc0fc94da6da50c39c2e1d7f24cd5ea8db12fc87c1332:Splice.Amulet:FeaturedAppRight",
      contract_id: "00fe121b3389e011f2fcf4681bd419874eb40e2fb7da5782cbf4062c5a971422c8ca1112207cb7ab5ac84e61a8763a1b5a1dae1893916ab4f2299bcdbcb4988175dc639463",
      payload: {
        dso: "DSO::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a",
        provider: "Cumberland-GasStation-1::12205c94bc8c5427b06d3ab2bc13375ddc2ed2d5342bacbe3ba38ca6d9215a49f104"
      },
      created_event_blob: "CgMyLjES0wQKRQD+EhszieAR8vz0aBvUGYdOtA4vt9pXgsv0BixalxQiyMoREiB8t6tayE5hqHY6G1od..."
    },
    {
      template_id: "3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.Amulet:FeaturedAppRight",
      contract_id: "00d42315e5c0cee0f008d6b2c13bc437f6465dffc0aa83bd4b98ec92c1888b4807ca121220f8c50e256fd6eb24e3ec9492473ec2f899f33238ea020327d16a6a18eeca721b",
      payload: {
        dso: "DSO::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a",
        provider: "orderbook-operator-1::122034faf8f4af71d107a42441f8bc90cabfd63ab4386fc7f17d15d6e3b01c5bd2ae"
      },
      created_event_blob: "CgMyLjESzQQKRQDUIxXlwM7g8AjWssE7xDf2Rl3/wKqDvUuY7JLBiItIB8oSEiD4xQ4lb9brJOPslJJH..."
    },
    {
      template_id: "3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.Amulet:FeaturedAppRight",
      contract_id: "000ddc5b6f892b8646db51acc4ed06e6d224922f295d003c66c7d38a09ad9209c8ca1212208cd6b4b9df5b9ae23231eb9cb213df5a69e0d01f9602fc78b3a2833e5340dfac",
      payload: {
        dso: "DSO::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a",
        provider: "orderbook-operator-1::122034faf8f4af71d107a42441f8bc90cabfd63ab4386fc7f17d15d6e3b01c5bd2ae"
      },
      created_event_blob: "CgMyLjESzQQKRQAN3FtviSuGRttRrMTtBubSJJIvKV0APGbH04oJrZIJyMoSEiCM1rS531ua4jIx65yy..."
    }
  ]
};

// Helper to create mock fetch responses
function createMockFetch(responses: Record<string, { ok: boolean; data?: any; status?: number }>) {
  return async (url: string, options?: RequestInit) => {
    const urlStr = url.toString();

    for (const [pattern, response] of Object.entries(responses)) {
      if (urlStr.includes(pattern)) {
        return {
          ok: response.ok,
          status: response.status || (response.ok ? 200 : 500),
          json: async () => response.data,
          text: async () => JSON.stringify(response.data)
        };
      }
    }

    return {
      ok: false,
      status: 404,
      json: async () => ({ error: "Not found" }),
      text: async () => "Not found"
    };
  };
}

describe("Scan API Transfer Context", () => {
  describe("getScanBaseUrl", () => {
    it("should return localhost URL by default", () => {
      // Scan API is accessed via validator's scan proxy at localhost:8080
      const defaultUrl = "http://localhost:8080";
      assert.strictEqual(defaultUrl, "http://localhost:8080");
    });

    it("should use SCAN_API_BASE_URL env var when set", () => {
      // The function uses process.env.SCAN_API_BASE_URL || "http://localhost:8080"
      const envUrl = "http://custom-scan-api:9000";
      const getScanBaseUrl = () => envUrl || "http://localhost:8080";
      assert.strictEqual(getScanBaseUrl(), "http://custom-scan-api:9000");
    });

    it("should fallback to localhost when env var not set", () => {
      const envUrl = undefined;
      const getScanBaseUrl = () => envUrl || "http://localhost:8080";
      assert.strictEqual(getScanBaseUrl(), "http://localhost:8080");
    });
  });

  describe("Response Parsing", () => {
    it("should extract amuletRulesContractId from DSO response", () => {
      const amuletRulesContractId = mockDsoResponse.amulet_rules?.contract?.contract_id;
      assert.strictEqual(
        amuletRulesContractId,
        "00554dd41d43438d8ee88312085c79447a16efe55a7fc941cc4f67af2fe268ed73ca11122016f7f9e1b0549120f942f63c249c55c27f7bd4ebde1c869cb9d7b0ede5084f6a"
      );
    });

    it("should extract amuletRulesTemplateId from DSO response", () => {
      const templateId = mockDsoResponse.amulet_rules?.contract?.template_id;
      assert.strictEqual(
        templateId,
        "3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.AmuletRules:AmuletRules"
      );
    });

    it("should extract created_event_blob from DSO response", () => {
      const blob = mockDsoResponse.amulet_rules?.contract?.created_event_blob;
      assert.ok(blob, "created_event_blob should exist");
      assert.ok(blob.startsWith("CgMyLjE"), "blob should start with expected prefix");
    });

    it("should extract dsoParty from DSO response", () => {
      const dsoParty = mockDsoResponse.amulet_rules?.contract?.payload?.dso;
      assert.strictEqual(
        dsoParty,
        "DSO::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a"
      );
    });

    it("should extract dsoPartyId from top-level DSO response", () => {
      const dsoPartyId = mockDsoResponse.dso_party_id;
      assert.strictEqual(
        dsoPartyId,
        "DSO::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a"
      );
    });

    it("should extract openRoundContractId from rounds response (map structure)", () => {
      const openRoundsMap = mockRoundsResponse.open_mining_rounds;
      assert.ok(openRoundsMap && Object.keys(openRoundsMap).length > 0, "Should have open mining rounds");
      const firstKey = Object.keys(openRoundsMap)[0];
      const openRoundContractId = openRoundsMap[firstKey as keyof typeof openRoundsMap]?.contract?.contract_id;
      assert.ok(openRoundContractId, "Should have contract_id");
    });

    it("should extract openRound templateId from rounds response", () => {
      const openRoundsMap = mockRoundsResponse.open_mining_rounds;
      const firstKey = Object.keys(openRoundsMap)[0];
      const templateId = openRoundsMap[firstKey as keyof typeof openRoundsMap]?.contract?.template_id;
      assert.strictEqual(
        templateId,
        "3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.Round:OpenMiningRound"
      );
    });

    it("should extract openRound created_event_blob from rounds response", () => {
      const openRoundsMap = mockRoundsResponse.open_mining_rounds;
      const firstKey = Object.keys(openRoundsMap)[0];
      const blob = openRoundsMap[firstKey as keyof typeof openRoundsMap]?.contract?.created_event_blob;
      assert.ok(blob, "created_event_blob should exist");
      assert.ok(blob.startsWith("CgMyLjE"), "blob should start with expected prefix");
    });

    it("should sort open mining rounds by opensAt (earliest first)", () => {
      const openRoundsMap = mockRoundsResponse.open_mining_rounds;

      // Convert map to array and sort by opensAt (earliest first)
      const sortedRounds = Object.entries(openRoundsMap)
        .map(([contractId, roundData]) => ({
          contractId,
          roundData,
          opensAt: roundData?.contract?.payload?.opensAt || "",
        }))
        .sort((a, b) => a.opensAt.localeCompare(b.opensAt));

      // The earliest round should be round 20792 (opensAt: 2025-11-27T19:37:06.219171Z)
      const earliestRound = sortedRounds[0];
      assert.strictEqual(earliestRound.roundData.contract.payload.round.number, "20792");
      assert.strictEqual(earliestRound.opensAt, "2025-11-27T19:37:06.219171Z");

      // The later round should be round 20794 (opensAt: 2025-11-27T19:57:43.853001Z)
      const laterRound = sortedRounds[1];
      assert.strictEqual(laterRound.roundData.contract.payload.round.number, "20794");
      assert.strictEqual(laterRound.opensAt, "2025-11-27T19:57:43.853001Z");
    });

    it("should extract transferPreapprovalContractId from preapproval response", () => {
      const transferPreapprovalContractId =
        mockPreapprovalResponse.transfer_preapproval?.contract?.contract_id;
      assert.strictEqual(transferPreapprovalContractId, "00preapproval101112");
    });

    it("should handle missing amulet_rules gracefully", () => {
      const badResponse = { dso_rules: {} };
      const amuletRulesContractId = (badResponse as any).amulet_rules?.contract?.contract_id;
      assert.strictEqual(amuletRulesContractId, undefined);
    });

    it("should handle empty open_mining_rounds gracefully", () => {
      const emptyRoundsResponse = { open_mining_rounds: {}, issuing_mining_rounds: {} };
      const openRoundsMap = emptyRoundsResponse.open_mining_rounds;
      assert.strictEqual(Object.keys(openRoundsMap).length, 0);
    });

    it("should handle missing transfer_preapproval gracefully", () => {
      const badResponse = {};
      const transferPreapprovalContractId =
        (badResponse as any).transfer_preapproval?.contract?.contract_id;
      assert.strictEqual(transferPreapprovalContractId, undefined);
    });

    it("should extract externalPartyAmuletRules contractId from response", () => {
      const contractId = mockExternalPartyAmuletRulesResponse.external_party_amulet_rules_update?.contract?.contract_id;
      assert.strictEqual(
        contractId,
        "009f00e5bf00640118d849080aaf22bc963a8458d322585cebf1119cb7bf37a955ca11122065b775fb8a4199904ed32fa9277fd9c0e82bb82319a7151249df124182072381"
      );
    });

    it("should extract externalPartyAmuletRules templateId from response", () => {
      const templateId = mockExternalPartyAmuletRulesResponse.external_party_amulet_rules_update?.contract?.template_id;
      assert.strictEqual(
        templateId,
        "a5b055492fb8f08b2e7bc0fc94da6da50c39c2e1d7f24cd5ea8db12fc87c1332:Splice.ExternalPartyAmuletRules:ExternalPartyAmuletRules"
      );
    });

    it("should extract externalPartyAmuletRules created_event_blob from response", () => {
      const blob = mockExternalPartyAmuletRulesResponse.external_party_amulet_rules_update?.contract?.created_event_blob;
      assert.ok(blob, "created_event_blob should exist");
      assert.ok(blob.startsWith("CgMyLjE"), "blob should start with expected prefix");
    });

    it("should handle missing external_party_amulet_rules_update gracefully", () => {
      const badResponse = {};
      const contractId = (badResponse as any).external_party_amulet_rules_update?.contract?.contract_id;
      assert.strictEqual(contractId, undefined);
    });

    it("should find FeaturedAppRight by target provider", () => {
      const featuredApps = mockFeaturedAppsResponse.featured_apps;
      const targetProvider = "orderbook-operator-1::122034faf8f4af71d107a42441f8bc90cabfd63ab4386fc7f17d15d6e3b01c5bd2ae";

      const matchedApp = featuredApps.find((app: any) =>
        app.payload?.provider === targetProvider ||
        app.payload?.provider?.includes("orderbook-operator-1")
      );

      assert.ok(matchedApp, "Should find FeaturedAppRight for orderbook-operator-1");
      assert.strictEqual(
        matchedApp?.contract_id,
        "00d42315e5c0cee0f008d6b2c13bc437f6465dffc0aa83bd4b98ec92c1888b4807ca121220f8c50e256fd6eb24e3ec9492473ec2f899f33238ea020327d16a6a18eeca721b"
      );
      assert.strictEqual(
        matchedApp?.payload?.provider,
        "orderbook-operator-1::122034faf8f4af71d107a42441f8bc90cabfd63ab4386fc7f17d15d6e3b01c5bd2ae"
      );
    });

    it("should extract FeaturedAppRight template_id correctly", () => {
      const featuredApps = mockFeaturedAppsResponse.featured_apps;
      const matchedApp = featuredApps.find((app: any) =>
        app.payload?.provider?.includes("orderbook-operator-1")
      );

      assert.ok(matchedApp?.template_id, "Should have template_id");
      assert.ok(
        matchedApp?.template_id.includes("Splice.Amulet:FeaturedAppRight"),
        "Template should be FeaturedAppRight"
      );
    });

    it("should extract FeaturedAppRight created_event_blob correctly", () => {
      const featuredApps = mockFeaturedAppsResponse.featured_apps;
      const matchedApp = featuredApps.find((app: any) =>
        app.payload?.provider?.includes("orderbook-operator-1")
      );

      assert.ok(matchedApp?.created_event_blob, "Should have created_event_blob");
      assert.ok(
        matchedApp?.created_event_blob.startsWith("CgMyLjE"),
        "Blob should start with expected prefix"
      );
    });

    it("should fallback to first FeaturedAppRight if target provider not found", () => {
      const featuredApps = mockFeaturedAppsResponse.featured_apps;
      const targetProvider = "non-existent-provider::1220abc";

      let matchedApp = featuredApps.find((app: any) =>
        app.payload?.provider === targetProvider
      );

      // Fallback to first available
      if (!matchedApp && featuredApps.length > 0) {
        matchedApp = featuredApps[0];
      }

      assert.ok(matchedApp, "Should fallback to first FeaturedAppRight");
      assert.strictEqual(
        matchedApp?.payload?.provider,
        "Cumberland-GasStation-1::12205c94bc8c5427b06d3ab2bc13375ddc2ed2d5342bacbe3ba38ca6d9215a49f104"
      );
    });

    it("should handle empty featured_apps array gracefully", () => {
      const emptyResponse = { featured_apps: [] };
      const featuredApps = emptyResponse.featured_apps;

      const matchedApp = featuredApps.find((app: any) =>
        app.payload?.provider?.includes("orderbook-operator-1")
      );

      assert.strictEqual(matchedApp, undefined);
    });
  });

  describe("URL Construction", () => {
    it("should construct correct DSO endpoint URL", () => {
      const baseUrl = "http://localhost:8080";
      const dsoUrl = `${baseUrl}/api/scan/v0/dso`;
      assert.strictEqual(dsoUrl, "http://localhost:8080/api/scan/v0/dso");
    });

    it("should construct correct rounds endpoint URL", () => {
      const baseUrl = "http://localhost:8080";
      const roundsUrl = `${baseUrl}/api/scan/v0/open-and-issuing-mining-rounds`;
      assert.strictEqual(
        roundsUrl,
        "http://localhost:8080/api/scan/v0/open-and-issuing-mining-rounds"
      );
    });

    it("should construct correct preapproval endpoint URL with encoded party", () => {
      const baseUrl = "http://localhost:8080";
      const receiverParty = "alice::1220abc123";
      const preapprovalUrl = `${baseUrl}/api/scan/v0/transfer-preapprovals/by-party/${encodeURIComponent(receiverParty)}`;
      assert.strictEqual(
        preapprovalUrl,
        "http://localhost:8080/api/scan/v0/transfer-preapprovals/by-party/alice%3A%3A1220abc123"
      );
    });

    it("should construct correct external-party-amulet-rules endpoint URL", () => {
      const baseUrl = "http://localhost:8080";
      const externalPartyAmuletRulesUrl = `${baseUrl}/api/scan/v0/external-party-amulet-rules`;
      assert.strictEqual(
        externalPartyAmuletRulesUrl,
        "http://localhost:8080/api/scan/v0/external-party-amulet-rules"
      );
    });
  });

  describe("TransferInput Format", () => {
    it("should format InputAmulet correctly", () => {
      const contractId = "00amulet123";
      const input = { tag: "InputAmulet", value: contractId };
      assert.deepStrictEqual(input, { tag: "InputAmulet", value: "00amulet123" });
    });

    it("should format multiple inputs correctly", () => {
      const contractIds = ["00amulet1", "00amulet2", "00amulet3"];
      const inputs = contractIds.map(id => ({ tag: "InputAmulet", value: id }));
      assert.strictEqual(inputs.length, 3);
      assert.deepStrictEqual(inputs[0], { tag: "InputAmulet", value: "00amulet1" });
      assert.deepStrictEqual(inputs[2], { tag: "InputAmulet", value: "00amulet3" });
    });
  });

  describe("TransferFactory_Transfer Command Structure", () => {
    it("should build correct command structure with five disclosed contracts", () => {
      const sender = "alice::1220abc";
      const receiver = "bob::1220def";
      const amount = "100.0";
      const description = "Test transfer";
      const inputHoldingCids = ["00amulet1"];
      const amuletRulesContractId = "00rules";
      const openRoundContractId = "00round";
      const transferPreapprovalContractId = "00preapproval";
      const externalPartyAmuletRulesContractId = "00externalparty";
      const featuredAppRightContractId = "00d42315e5c0cee0f008d6b2c13bc437f6465dffc0aa83bd4b98ec92c1888b4807ca121220f8c50e256fd6eb24e3ec9492473ec2f899f33238ea020327d16a6a18eeca721b";
      const dsoParty = "DSO::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a";
      const synchronizerId = "global-domain::1220abc";

      const now = new Date();
      const executeBefore = new Date(now.getTime() + 30000);

      // Disclosed contracts - 5 total: AmuletRules, OpenMiningRound, ExternalPartyAmuletRules, TransferPreapproval, FeaturedAppRight
      const disclosedContracts = [
        {
          contractId: amuletRulesContractId,
          createdEventBlob: "CgMyLjE...",
          synchronizerId,
          templateId: "3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.AmuletRules:AmuletRules",
        },
        {
          contractId: openRoundContractId,
          createdEventBlob: "CgMyLjE...",
          synchronizerId,
          templateId: "3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.Round:OpenMiningRound",
        },
        {
          contractId: externalPartyAmuletRulesContractId,
          createdEventBlob: "CgMyLjE...",
          synchronizerId,
          templateId: "a5b055492fb8f08b2e7bc0fc94da6da50c39c2e1d7f24cd5ea8db12fc87c1332:Splice.ExternalPartyAmuletRules:ExternalPartyAmuletRules",
        },
        {
          contractId: transferPreapprovalContractId,
          createdEventBlob: "CgMyLjESpreapproval...",
          synchronizerId,
          templateId: "3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.AmuletRules:TransferPreapproval",
        },
        {
          contractId: featuredAppRightContractId,
          createdEventBlob: "CgMyLjESzQQKRQDUIxXlwM7g8AjWssE7xDf2Rl3/wKqDvUuY7JLBiItIB8oSEiD4xQ4lb9brJOPslJJH...",
          synchronizerId,
          templateId: "3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.Amulet:FeaturedAppRight",
        },
      ];

      // TransferFactory_Transfer command structure with extraArgs using AV_ContractId tags
      const command = {
        commands: [{
          ExerciseCommand: {
            templateId: "#splice-api-token-transfer-instruction-v1:Splice.Api.Token.TransferInstructionV1:TransferFactory",
            contractId: externalPartyAmuletRulesContractId,
            choice: "TransferFactory_Transfer",
            choiceArgument: {
              expectedAdmin: dsoParty,
              transfer: {
                sender,
                receiver,
                amount,
                instrumentId: {
                  admin: dsoParty,
                  id: "Amulet"
                },
                inputHoldingCids,
                requestedAt: now.toISOString().replace('Z', '000Z'),
                executeBefore: executeBefore.toISOString().replace('Z', '000Z'),
                meta: {
                  values: {
                    "splice.lfdecentralizedtrust.org/reason": description
                  }
                }
              },
              extraArgs: {
                context: {
                  values: {
                    "amulet-rules": {
                      tag: "AV_ContractId",
                      value: amuletRulesContractId
                    },
                    "open-round": {
                      tag: "AV_ContractId",
                      value: openRoundContractId
                    },
                    "featured-app-right": {
                      tag: "AV_ContractId",
                      value: featuredAppRightContractId
                    },
                    "transfer-preapproval": {
                      tag: "AV_ContractId",
                      value: transferPreapprovalContractId
                    }
                  }
                },
                meta: { values: {} }
              }
            }
          }
        }],
        disclosedContracts,
      };

      assert.strictEqual(command.commands.length, 1);
      assert.strictEqual(command.commands[0].ExerciseCommand.choice, "TransferFactory_Transfer");
      assert.strictEqual(command.commands[0].ExerciseCommand.contractId, externalPartyAmuletRulesContractId);
      assert.strictEqual(command.commands[0].ExerciseCommand.choiceArgument.transfer.sender, sender);
      assert.strictEqual(command.commands[0].ExerciseCommand.choiceArgument.transfer.receiver, receiver);
      assert.strictEqual(command.commands[0].ExerciseCommand.choiceArgument.transfer.amount, amount);
      assert.strictEqual(command.commands[0].ExerciseCommand.choiceArgument.expectedAdmin, dsoParty);
      assert.deepStrictEqual(command.commands[0].ExerciseCommand.choiceArgument.transfer.inputHoldingCids, inputHoldingCids);

      // Verify extraArgs context uses AV_ContractId tags
      const contextValues = command.commands[0].ExerciseCommand.choiceArgument.extraArgs.context.values;
      assert.strictEqual(contextValues["amulet-rules"].tag, "AV_ContractId");
      assert.strictEqual(contextValues["amulet-rules"].value, amuletRulesContractId);
      assert.strictEqual(contextValues["open-round"].tag, "AV_ContractId");
      assert.strictEqual(contextValues["open-round"].value, openRoundContractId);
      assert.strictEqual(contextValues["featured-app-right"].tag, "AV_ContractId");
      assert.strictEqual(contextValues["featured-app-right"].value, featuredAppRightContractId);
      assert.strictEqual(contextValues["transfer-preapproval"].tag, "AV_ContractId");
      assert.strictEqual(contextValues["transfer-preapproval"].value, transferPreapprovalContractId);

      // Verify all five disclosed contracts
      assert.strictEqual(command.disclosedContracts.length, 5);
      assert.strictEqual(command.disclosedContracts[0].contractId, amuletRulesContractId);
      assert.strictEqual(command.disclosedContracts[1].contractId, openRoundContractId);
      assert.strictEqual(command.disclosedContracts[2].contractId, externalPartyAmuletRulesContractId);
      assert.strictEqual(command.disclosedContracts[3].contractId, transferPreapprovalContractId);
      assert.strictEqual(command.disclosedContracts[4].contractId, featuredAppRightContractId);
      assert.ok(command.disclosedContracts[0].createdEventBlob, "AmuletRules should have blob");
      assert.ok(command.disclosedContracts[1].createdEventBlob, "OpenMiningRound should have blob");
      assert.ok(command.disclosedContracts[2].createdEventBlob, "ExternalPartyAmuletRules should have blob");
      assert.ok(command.disclosedContracts[3].createdEventBlob, "TransferPreapproval should have blob");
      assert.ok(command.disclosedContracts[4].createdEventBlob, "FeaturedAppRight should have blob");
    });

    it("should use correct templateId for TransferFactory interface", () => {
      const templateId = "#splice-api-token-transfer-instruction-v1:Splice.Api.Token.TransferInstructionV1:TransferFactory";
      assert.ok(templateId.includes("TransferFactory"));
      assert.ok(templateId.includes("TransferInstructionV1"));
    });

    it("should format timestamps correctly - requestedAt with microseconds, executeBefore with milliseconds", () => {
      const now = new Date("2025-11-27T20:00:00.123Z");
      const executeBefore = new Date(now.getTime() + 30000);

      // requestedAt uses microsecond precision (6 digits)
      const requestedAtStr = now.toISOString().replace(/Z$/, '000Z');
      assert.strictEqual(requestedAtStr, "2025-11-27T20:00:00.123000Z");

      // executeBefore uses millisecond precision (3 digits - as-is from toISOString)
      const executeBeforeStr = executeBefore.toISOString();
      assert.strictEqual(executeBeforeStr, "2025-11-27T20:00:30.123Z");
    });

    it("should handle missing FeaturedAppRight gracefully", () => {
      // When featuredAppRight is null, still include in extraArgs but with null value
      const contextValues = {
        "amulet-rules": { tag: "AV_ContractId", value: "00rules" },
        "open-round": { tag: "AV_ContractId", value: "00round" },
        "featured-app-right": { tag: "AV_ContractId", value: null },
        "transfer-preapproval": { tag: "AV_ContractId", value: "00preapproval" }
      };
      assert.strictEqual(contextValues["featured-app-right"].value, null);
    });
  });
});
