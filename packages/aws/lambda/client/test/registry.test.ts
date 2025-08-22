import { describe, it } from "node:test";
import assert from "node:assert";
import { LambdaClient } from "../src/index.js";
import * as dotenv from "dotenv";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

// Load environment variables
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
dotenv.config({ path: join(__dirname, "../.env") });

// Helper function to format date for registry names
function formatDateForRegistry(): string {
  const now = new Date();
  return now
    .toISOString()
    .replace(/T/, " ")
    .replace(/\..+/, "")
    .replace(/:/g, "-");
}

describe("Registry Management", () => {
  const client = new LambdaClient(process.env.LAMBDA_API_URL);

  // Store registry ID for subsequent tests
  let testRegistryId: string | undefined;
  let testDeveloperName: string;
  let testAgentName: string;
  let testAppName: string;

  describe("createRegistry", () => {
    it("should create a new registry on testnet", async () => {
      const registryName = `Test Registry ${formatDateForRegistry()}`;
      const chain = "testnet";

      console.log(`Creating registry "${registryName}" on ${chain}...`);

      const response = await client.createRegistry(registryName, chain);

      assert.ok(response.registry_id, "Response should contain a registry_id");
      assert.strictEqual(
        typeof response.registry_id,
        "string",
        "Registry ID should be a string"
      );
      assert.ok(
        response.registry_id.startsWith("0x"),
        "Registry ID should start with 0x"
      );

      assert.ok(response.tx_digest, "Response should contain a tx_digest");
      assert.strictEqual(
        typeof response.tx_digest,
        "string",
        "Transaction digest should be a string"
      );

      assert.ok(
        response.admin_address,
        "Response should contain an admin_address"
      );
      assert.strictEqual(
        typeof response.admin_address,
        "string",
        "Admin address should be a string"
      );
      assert.ok(
        response.admin_address.startsWith("0x"),
        "Admin address should start with 0x"
      );
      assert.strictEqual(
        response.admin_address.length,
        66,
        "Admin address should be 66 characters (0x + 64 hex chars)"
      );

      console.log(`Registry created successfully!`);
      console.log(`  Registry ID: ${response.registry_id}`);
      console.log(`  Transaction: ${response.tx_digest}`);
      console.log(`  Admin: ${response.admin_address}`);

      // Store registry ID for subsequent tests
      testRegistryId = response.registry_id;
    });

    it(
      "should handle registry creation errors gracefully",
      { skip: true },
      async () => {
        // This test would test error conditions but is skipped by default
        // since we don't want to waste gas on failed transactions

        const registryName = ""; // Empty name should fail
        const chain = "testnet";

        try {
          await client.createRegistry(registryName, chain);
          assert.fail("Should have thrown an error for empty registry name");
        } catch (error: any) {
          assert.ok(
            error.message.includes("API Error"),
            "Error should be an API error"
          );
          console.log(`Expected error for empty name: ${error.message}`);
        }
      }
    );

    it("should create registry with long names", { skip: true }, async () => {
      // Skip by default to avoid creating too many test registries
      const registryName = `Very Long Test Registry Name That Should Still Work ${formatDateForRegistry()}`;
      const chain = "testnet";

      const response = await client.createRegistry(registryName, chain);

      assert.ok(
        response.registry_id,
        "Should successfully create registry with long name"
      );
      console.log(`Created registry with long name: ${response.registry_id}`);
    });
  });

  describe("Developer Management", () => {
    it("should add a developer to the registry", async () => {
      if (!testRegistryId) {
        console.log(
          "Skipping: No registry ID available. Run createRegistry test first."
        );
        return;
      }

      testDeveloperName = `TestDev_${Date.now()}`;
      const response = await client.addDeveloper({
        registry_id: testRegistryId,
        chain: "testnet",
        name: testDeveloperName,
        github: "testdev",
        image: "https://example.com/avatar.png",
        description: "Test developer account",
        site: "https://example.com",
      });

      assert.ok(response.tx_digest, "Response should contain a tx_digest");
      assert.strictEqual(
        typeof response.tx_digest,
        "string",
        "Transaction digest should be a string"
      );
      console.log(
        `Developer '${testDeveloperName}' added with tx: ${response.tx_digest}`
      );
    });

    it("should update a developer in the registry", async () => {
      if (!testRegistryId || !testDeveloperName) {
        console.log("Skipping: Prerequisites not met.");
        return;
      }

      const response = await client.updateDeveloper({
        registry_id: testRegistryId,
        chain: "testnet",
        name: testDeveloperName,
        github: "testdev-updated",
        image: "https://example.com/avatar2.png",
        description: "Updated test developer account",
        site: "https://updated.example.com",
      });

      assert.ok(response.tx_digest, "Response should contain a tx_digest");
      console.log(
        `Developer '${testDeveloperName}' updated with tx: ${response.tx_digest}`
      );
    });
  });

  describe("Agent Management", () => {
    it("should add an agent to the registry", async () => {
      if (!testRegistryId || !testDeveloperName) {
        console.log("Skipping: Prerequisites not met.");
        return;
      }

      testAgentName = `TestAgent_${Date.now()}`;
      const response = await client.addAgent({
        registry_id: testRegistryId,
        chain: "testnet",
        developer: testDeveloperName,
        name: testAgentName,
        image: "https://example.com/agent.png",
        description: "Test agent for automation",
        site: "https://agent.example.com",
        chains: ["sui-testnet", "sui-testnet"],
      });

      assert.ok(response.tx_digest, "Response should contain a tx_digest");
      console.log(
        `Agent '${testAgentName}' added with tx: ${response.tx_digest}`
      );
    });

    it("should update an agent in the registry", async () => {
      if (!testRegistryId || !testDeveloperName || !testAgentName) {
        console.log("Skipping: Prerequisites not met.");
        return;
      }

      const response = await client.updateAgent({
        registry_id: testRegistryId,
        chain: "testnet",
        developer: testDeveloperName,
        name: testAgentName,
        image: "https://example.com/agent2.png",
        description: "Updated test agent",
        site: "https://agent2.example.com",
        chains: ["sui-testnet", "sui-testnet", "sui-mainnet"],
      });

      assert.ok(response.tx_digest, "Response should contain a tx_digest");
      console.log(
        `Agent '${testAgentName}' updated with tx: ${response.tx_digest}`
      );
    });

    it("should remove an agent from the registry", { skip: true }, async () => {
      // Skip by default to preserve test data
      if (!testRegistryId || !testDeveloperName || !testAgentName) {
        console.log("Skipping: Prerequisites not met.");
        return;
      }

      const response = await client.removeAgent({
        registry_id: testRegistryId,
        chain: "testnet",
        developer: testDeveloperName,
        name: testAgentName,
      });

      assert.ok(response.tx_digest, "Response should contain a tx_digest");
      console.log(
        `Agent '${testAgentName}' removed with tx: ${response.tx_digest}`
      );
    });
  });

  describe("App Management", () => {
    it("should add an app to the registry", async () => {
      if (!testRegistryId) {
        console.log("Skipping: No registry ID available.");
        return;
      }

      testAppName = `TestApp_${Date.now()}`;
      const response = await client.addApp({
        registry_id: testRegistryId,
        chain: "testnet",
        name: testAppName,
        description: "Test application",
        image: "https://example.com/app.png",
        site: "https://app.example.com",
        app_cap: null, // Optional app capability
      });

      assert.ok(response.tx_digest, "Response should contain a tx_digest");
      console.log(`App '${testAppName}' added with tx: ${response.tx_digest}`);
    });

    it("should update an app in the registry", async () => {
      if (!testRegistryId || !testAppName) {
        console.log("Skipping: Prerequisites not met.");
        return;
      }

      const response = await client.updateApp({
        registry_id: testRegistryId,
        chain: "testnet",
        name: testAppName,
        description: "Updated test application",
        image: "https://example.com/app2.png",
        site: "https://app2.example.com",
      });

      assert.ok(response.tx_digest, "Response should contain a tx_digest");
      console.log(
        `App '${testAppName}' updated with tx: ${response.tx_digest}`
      );
    });

    it("should remove an app from the registry", { skip: true }, async () => {
      // Skip by default to preserve test data
      if (!testRegistryId || !testAppName) {
        console.log("Skipping: Prerequisites not met.");
        return;
      }

      const response = await client.removeApp({
        registry_id: testRegistryId,
        chain: "testnet",
        name: testAppName,
      });

      assert.ok(response.tx_digest, "Response should contain a tx_digest");
      console.log(
        `App '${testAppName}' removed with tx: ${response.tx_digest}`
      );
    });
  });

  describe("Developer Removal", () => {
    it("should remove a developer and its agents", { skip: true }, async () => {
      // Skip by default to preserve test data
      if (!testRegistryId || !testDeveloperName) {
        console.log("Skipping: Prerequisites not met.");
        return;
      }

      const response = await client.removeDeveloper({
        registry_id: testRegistryId,
        chain: "testnet",
        name: testDeveloperName,
        agent_names: testAgentName ? [testAgentName] : [],
      });

      assert.ok(response.tx_digest, "Response should contain a tx_digest");
      console.log(
        `Developer '${testDeveloperName}' removed with tx: ${response.tx_digest}`
      );
    });
  });

  describe("Error Handling", () => {
    it("should handle invalid registry ID gracefully", async () => {
      try {
        await client.addDeveloper({
          registry_id: "0xinvalid",
          chain: "testnet",
          name: "TestDev",
          github: "test",
        });
        assert.fail("Should have thrown an error for invalid registry ID");
      } catch (error: any) {
        assert.ok(error.message, "Error should have a message");
        console.log(`Expected error for invalid registry: ${error.message}`);
      }
    });

    it("should handle duplicate names gracefully", { skip: true }, async () => {
      // This would test adding duplicate developers/agents/apps
      // Skip by default as it depends on having specific test data
      if (!testRegistryId || !testDeveloperName) {
        console.log("Skipping: Prerequisites not met.");
        return;
      }

      try {
        await client.addDeveloper({
          registry_id: testRegistryId,
          chain: "testnet",
          name: testDeveloperName, // Same name as existing
          github: "duplicate",
        });
        assert.fail("Should have thrown an error for duplicate developer");
      } catch (error: any) {
        assert.ok(error.message, "Error should have a message");
        console.log(`Expected error for duplicate: ${error.message}`);
      }
    });
  });
});
