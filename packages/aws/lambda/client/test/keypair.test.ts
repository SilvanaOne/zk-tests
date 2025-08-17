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

  describe('signMessage', () => {
    it('should sign a message and return signature with address', async () => {
      const loginType = 'email';
      const login = 'signer@example.com';
      
      // First ensure we have a keypair
      const keypairResponse = await client.generateSuiKeypair(loginType, login);
      assert.ok(keypairResponse.address, 'Should have an address');
      
      // Create a message to sign (Hello World in bytes)
      const message = [72, 101, 108, 108, 111, 32, 87, 111, 114, 108, 100];
      
      // Sign the message
      const signResponse = await client.signMessage(loginType, login, message);
      
      assert.ok(signResponse.signature, 'Response should contain a signature');
      assert.ok(Array.isArray(signResponse.signature), 'Signature should be an array');
      assert.strictEqual(signResponse.signature.length, 97, 'Sui signature should be 97 bytes (flag + sig + pubkey)');
      assert.strictEqual(signResponse.signature[0], 0, 'First byte should be 0 (Ed25519 flag)');
      
      assert.ok(signResponse.address, 'Response should contain an address');
      assert.strictEqual(signResponse.address, keypairResponse.address, 'Address should match the keypair address');
      
      console.log(`Signed message for ${signResponse.address}`);
      console.log(`Signature length: ${signResponse.signature.length} bytes`);
    });

    it('should sign different messages with same keypair', async () => {
      const loginType = 'github';
      const login = 'multi-signer';
      
      // Ensure we have a keypair
      await client.generateSuiKeypair(loginType, login);
      
      // Sign first message
      const message1 = [72, 101, 108, 108, 111]; // "Hello"
      const response1 = await client.signMessage(loginType, login, message1);
      
      // Sign second message
      const message2 = [87, 111, 114, 108, 100]; // "World"
      const response2 = await client.signMessage(loginType, login, message2);
      
      // Same address, different signatures
      assert.strictEqual(response1.address, response2.address, 'Should use same address');
      assert.notDeepStrictEqual(response1.signature, response2.signature, 'Different messages should have different signatures');
      
      console.log(`Signed two different messages with address: ${response1.address}`);
    });

    it('should handle empty message', async () => {
      const loginType = 'email';
      const login = 'empty-signer@example.com';
      
      // Ensure we have a keypair
      await client.generateSuiKeypair(loginType, login);
      
      // Sign empty message
      const emptyMessage: number[] = [];
      const response = await client.signMessage(loginType, login, emptyMessage);
      
      assert.ok(response.signature, 'Should sign empty message');
      assert.strictEqual(response.signature.length, 97, 'Signature should still be 97 bytes');
      
      console.log(`Signed empty message with address: ${response.address}`);
    });

    it('should fail for non-existent keypair', async () => {
      const loginType = 'email';
      const login = `nonexistent-${Date.now()}@example.com`;
      
      // Try to sign without generating keypair first
      const message = [1, 2, 3];
      
      try {
        await client.signMessage(loginType, login, message);
        assert.fail('Should have thrown an error for non-existent keypair');
      } catch (error: any) {
        assert.ok(error.message.includes('Keypair not found') || error.message.includes('API Error'), 
          'Should indicate keypair not found');
        console.log(`Expected error for non-existent keypair: ${error.message}`);
      }
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