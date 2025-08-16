import { describe, it } from 'node:test';
import assert from 'node:assert';
import { LambdaClient } from '../src/index.js';
import * as dotenv from 'dotenv';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

// Load environment variables
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
dotenv.config({ path: join(__dirname, '../.env') });

// Helper function to format date for registry names
function formatDateForRegistry(): string {
  const now = new Date();
  return now.toISOString()
    .replace(/T/, ' ')
    .replace(/\..+/, '')
    .replace(/:/g, '-');
}

describe('Registry Creation', () => {
  const client = new LambdaClient(process.env.LAMBDA_API_URL);

  describe('createRegistry', () => {
    it('should create a new registry on devnet', async () => {
      const registryName = `Test Registry ${formatDateForRegistry()}`;
      const chain = 'devnet';
      
      console.log(`Creating registry "${registryName}" on ${chain}...`);
      
      const response = await client.createRegistry(registryName, chain);
      
      assert.ok(response.registry_id, 'Response should contain a registry_id');
      assert.strictEqual(typeof response.registry_id, 'string', 'Registry ID should be a string');
      assert.ok(response.registry_id.startsWith('0x'), 'Registry ID should start with 0x');
      
      assert.ok(response.tx_digest, 'Response should contain a tx_digest');
      assert.strictEqual(typeof response.tx_digest, 'string', 'Transaction digest should be a string');
      
      assert.ok(response.admin_address, 'Response should contain an admin_address');
      assert.strictEqual(typeof response.admin_address, 'string', 'Admin address should be a string');
      assert.ok(response.admin_address.startsWith('0x'), 'Admin address should start with 0x');
      assert.strictEqual(response.admin_address.length, 66, 'Admin address should be 66 characters (0x + 64 hex chars)');
      
      console.log(`Registry created successfully!`);
      console.log(`  Registry ID: ${response.registry_id}`);
      console.log(`  Transaction: ${response.tx_digest}`);
      console.log(`  Admin: ${response.admin_address}`);
    });

    it('should handle registry creation errors gracefully', { skip: true }, async () => {
      // This test would test error conditions but is skipped by default
      // since we don't want to waste gas on failed transactions
      
      const registryName = '';  // Empty name should fail
      const chain = 'devnet';
      
      try {
        await client.createRegistry(registryName, chain);
        assert.fail('Should have thrown an error for empty registry name');
      } catch (error: any) {
        assert.ok(error.message.includes('API Error'), 'Error should be an API error');
        console.log(`Expected error for empty name: ${error.message}`);
      }
    });

    it('should create registry with long names', { skip: true }, async () => {
      // Skip by default to avoid creating too many test registries
      const registryName = `Very Long Test Registry Name That Should Still Work ${formatDateForRegistry()}`;
      const chain = 'devnet';
      
      const response = await client.createRegistry(registryName, chain);
      
      assert.ok(response.registry_id, 'Should successfully create registry with long name');
      console.log(`Created registry with long name: ${response.registry_id}`);
    });
  });
});