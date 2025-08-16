import { LambdaClient } from '../src/index.js';
import * as dotenv from 'dotenv';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

// Load environment variables
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
dotenv.config({ path: join(__dirname, '../.env') });

async function testClient() {
  console.log('🚀 Testing Lambda Client\n');
  
  const apiUrl = process.env.LAMBDA_API_URL || 'https://wprchl5wjd.execute-api.us-east-1.amazonaws.com/stage';
  console.log(`API URL: ${apiUrl}\n`);
  
  const client = new LambdaClient(apiUrl);
  
  try {
    // Test 1: Add operation
    console.log('Test 1: Adding 10 + 20');
    const addResult = await client.add(10, 20);
    console.log('Result:', addResult);
    console.log(`✅ Addition successful: ${addResult.result}\n`);
    
    // Test 2: Multiply operation
    console.log('Test 2: Multiplying 5 * 6');
    const multiplyResult = await client.multiply(5, 6);
    console.log('Result:', multiplyResult);
    console.log(`✅ Multiplication successful: ${multiplyResult.result}\n`);
    
    // Test 3: Generate Sui keypair
    console.log('Test 3: Generating Sui keypair');
    const keypairResult = await client.generateSuiKeypair('email', 'test@example.com');
    console.log('Result:', keypairResult);
    console.log(`✅ Keypair generated/retrieved: ${keypairResult.address}\n`);
    
    // Test 4: Retrieve same keypair
    console.log('Test 4: Retrieving same keypair');
    const keypairResult2 = await client.generateSuiKeypair('email', 'test@example.com');
    console.log('Result:', keypairResult2);
    console.log(`✅ Same address returned: ${keypairResult2.address === keypairResult.address}\n`);
    
    console.log('🎉 All tests passed!');
    
  } catch (error) {
    console.error('❌ Test failed:', error);
    process.exit(1);
  }
}

// Run the tests
testClient().catch(console.error);