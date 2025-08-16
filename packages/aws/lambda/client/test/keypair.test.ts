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

describe('LambdaClient', () => {
  const client = new LambdaClient(process.env.LAMBDA_API_URL);

  describe('generateSuiKeypair', () => {
    it('should generate a new keypair for a new user', async () => {
      const loginType = 'email';
      const login = `test-${Date.now()}@example.com`;
      
      const response = await client.generateSuiKeypair(loginType, login);
      
      assert.ok(response.address, 'Response should contain an address');
      assert.strictEqual(typeof response.address, 'string', 'Address should be a string');
      assert.ok(response.address.startsWith('0x'), 'Address should start with 0x');
      assert.strictEqual(response.address.length, 66, 'Sui address should be 66 characters (0x + 64 hex chars)');
      
      console.log(`Generated Sui address: ${response.address}`);
    });

    it('should return the same address for an existing user', async () => {
      const loginType = 'github';
      const login = 'testuser123';
      
      // First call - generate new keypair
      const response1 = await client.generateSuiKeypair(loginType, login);
      assert.ok(response1.address, 'First response should contain an address');
      
      // Second call - should return the same address
      const response2 = await client.generateSuiKeypair(loginType, login);
      assert.ok(response2.address, 'Second response should contain an address');
      
      assert.strictEqual(response1.address, response2.address, 
        'Should return the same address for the same login credentials');
      
      console.log(`Consistent address for ${loginType}:${login}: ${response1.address}`);
    });

    it('should generate different addresses for different users', async () => {
      const response1 = await client.generateSuiKeypair('email', `user1-${Date.now()}@example.com`);
      const response2 = await client.generateSuiKeypair('email', `user2-${Date.now()}@example.com`);
      
      assert.ok(response1.address, 'First response should contain an address');
      assert.ok(response2.address, 'Second response should contain an address');
      assert.notStrictEqual(response1.address, response2.address, 
        'Different users should have different addresses');
      
      console.log(`User 1 address: ${response1.address}`);
      console.log(`User 2 address: ${response2.address}`);
    });
  });

  describe('math operations', () => {
    it('should add two numbers', async () => {
      const response = await client.add(5, 3);
      
      assert.strictEqual(response.result, 8, 'Should correctly add 5 + 3 = 8');
      assert.strictEqual(response.operation, 'add', 'Operation should be "add"');
      
      // If blockchain is configured, there might be a tx_hash
      if (response.tx_hash) {
        console.log(`Transaction hash: ${response.tx_hash}`);
      }
    });

    it('should multiply two numbers', async () => {
      const response = await client.multiply(4, 7);
      
      assert.strictEqual(response.result, 28, 'Should correctly multiply 4 * 7 = 28');
      assert.strictEqual(response.operation, 'multiply', 'Operation should be "multiply"');
    });

    it('should handle large numbers', async () => {
      const a = 1000000;
      const b = 2000000;
      
      const addResponse = await client.add(a, b);
      assert.strictEqual(addResponse.result, 3000000, 'Should handle large addition');
      
      const multiplyResponse = await client.multiply(1000, 1000);
      assert.strictEqual(multiplyResponse.result, 1000000, 'Should handle large multiplication');
    });
  });
});