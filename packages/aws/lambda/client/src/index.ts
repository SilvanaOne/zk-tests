import { client } from './generated/client.gen.js';
import { generateSuiKeypair, addNumbers, multiplyNumbers, createRegistry } from './generated/sdk.gen.js';
import type { 
  SuiKeypairRequest, 
  SuiKeypairResponse,
  MathRequest,
  MathResponse,
  CreateRegistryRequest,
  CreateRegistryResponse
} from './generated/types.gen.js';

export class LambdaClient {
  constructor(private baseUrl?: string) {
    if (baseUrl) {
      client.setConfig({
        baseUrl: baseUrl
      });
    } else if (process.env.LAMBDA_API_URL) {
      client.setConfig({
        baseUrl: process.env.LAMBDA_API_URL
      });
    }
  }

  /**
   * Add two numbers together
   */
  async add(a: number, b: number) {
    const response = await addNumbers({
      body: { a, b }
    });
    
    if (response.error) {
      throw new Error(`API Error: ${JSON.stringify(response.error)}`);
    }
    
    return response.data as MathResponse;
  }

  /**
   * Multiply two numbers
   */
  async multiply(a: number, b: number) {
    const response = await multiplyNumbers({
      body: { a, b }
    });
    
    if (response.error) {
      throw new Error(`API Error: ${JSON.stringify(response.error)}`);
    }
    
    return response.data as MathResponse;
  }

  /**
   * Generate or retrieve a Sui keypair for a specific login
   */
  async generateSuiKeypair(loginType: string, login: string) {
    const response = await generateSuiKeypair({
      body: { 
        login_type: loginType,
        login: login
      }
    });
    
    if (response.error) {
      throw new Error(`API Error: ${JSON.stringify(response.error)}`);
    }
    
    return response.data as SuiKeypairResponse;
  }

  /**
   * Create a Silvana registry on the Sui blockchain
   */
  async createRegistry(name: string, chain: 'devnet' | 'testnet' | 'mainnet') {
    const response = await createRegistry({
      body: { 
        name: name,
        chain: chain
      }
    });
    
    if (response.error) {
      throw new Error(`API Error: ${JSON.stringify(response.error)}`);
    }
    
    return response.data as CreateRegistryResponse;
  }
}

// Re-export types
export type {
  SuiKeypairRequest,
  SuiKeypairResponse,
  MathRequest,
  MathResponse,
  CreateRegistryRequest,
  CreateRegistryResponse
} from './generated/types.gen.js';