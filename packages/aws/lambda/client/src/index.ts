import { client } from './generated/client.gen.js';
import { 
  generateSuiKeypair, 
  signMessage,
  addNumbers, 
  multiplyNumbers, 
  createRegistry,
  addDeveloper,
  updateDeveloper,
  removeDeveloper,
  addAgent,
  updateAgent,
  removeAgent,
  addApp,
  updateApp,
  removeApp
} from './generated/sdk.gen.js';
import type { 
  SuiKeypairRequest, 
  SuiKeypairResponse,
  SignMessageRequest,
  SignMessageResponse,
  MathRequest,
  MathResponse,
  CreateRegistryRequest,
  CreateRegistryResponse,
  TransactionResponse,
  AddDeveloperRequest,
  UpdateDeveloperRequest,
  RemoveDeveloperRequest,
  AddAgentRequest,
  UpdateAgentRequest,
  RemoveAgentRequest,
  AddAppRequest,
  UpdateAppRequest,
  RemoveAppRequest
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
   * Sign a message with a Sui keypair
   */
  async signMessage(loginType: string, login: string, message: number[]) {
    const response = await signMessage({
      body: { 
        login_type: loginType,
        login: login,
        message: message
      }
    });
    
    if (response.error) {
      throw new Error(`API Error: ${JSON.stringify(response.error)}`);
    }
    
    return response.data as SignMessageResponse;
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

  /**
   * Add a developer to the registry
   */
  async addDeveloper(request: AddDeveloperRequest) {
    const response = await addDeveloper({
      body: request
    });
    
    if (response.error) {
      throw new Error(`API Error: ${JSON.stringify(response.error)}`);
    }
    
    return response.data as TransactionResponse;
  }

  /**
   * Update a developer in the registry
   */
  async updateDeveloper(request: UpdateDeveloperRequest) {
    const response = await updateDeveloper({
      body: request
    });
    
    if (response.error) {
      throw new Error(`API Error: ${JSON.stringify(response.error)}`);
    }
    
    return response.data as TransactionResponse;
  }

  /**
   * Remove a developer from the registry
   */
  async removeDeveloper(request: RemoveDeveloperRequest) {
    const response = await removeDeveloper({
      body: request
    });
    
    if (response.error) {
      throw new Error(`API Error: ${JSON.stringify(response.error)}`);
    }
    
    return response.data as TransactionResponse;
  }

  /**
   * Add an agent to the registry
   */
  async addAgent(request: AddAgentRequest) {
    const response = await addAgent({
      body: request
    });
    
    if (response.error) {
      throw new Error(`API Error: ${JSON.stringify(response.error)}`);
    }
    
    return response.data as TransactionResponse;
  }

  /**
   * Update an agent in the registry
   */
  async updateAgent(request: UpdateAgentRequest) {
    const response = await updateAgent({
      body: request
    });
    
    if (response.error) {
      throw new Error(`API Error: ${JSON.stringify(response.error)}`);
    }
    
    return response.data as TransactionResponse;
  }

  /**
   * Remove an agent from the registry
   */
  async removeAgent(request: RemoveAgentRequest) {
    const response = await removeAgent({
      body: request
    });
    
    if (response.error) {
      throw new Error(`API Error: ${JSON.stringify(response.error)}`);
    }
    
    return response.data as TransactionResponse;
  }

  /**
   * Add an app to the registry
   */
  async addApp(request: AddAppRequest) {
    const response = await addApp({
      body: request
    });
    
    if (response.error) {
      throw new Error(`API Error: ${JSON.stringify(response.error)}`);
    }
    
    return response.data as TransactionResponse;
  }

  /**
   * Update an app in the registry
   */
  async updateApp(request: UpdateAppRequest) {
    const response = await updateApp({
      body: request
    });
    
    if (response.error) {
      throw new Error(`API Error: ${JSON.stringify(response.error)}`);
    }
    
    return response.data as TransactionResponse;
  }

  /**
   * Remove an app from the registry
   */
  async removeApp(request: RemoveAppRequest) {
    const response = await removeApp({
      body: request
    });
    
    if (response.error) {
      throw new Error(`API Error: ${JSON.stringify(response.error)}`);
    }
    
    return response.data as TransactionResponse;
  }
}

// Re-export types
export type {
  SuiKeypairRequest,
  SuiKeypairResponse,
  SignMessageRequest,
  SignMessageResponse,
  MathRequest,
  MathResponse,
  CreateRegistryRequest,
  CreateRegistryResponse,
  TransactionResponse,
  AddDeveloperRequest,
  UpdateDeveloperRequest,
  RemoveDeveloperRequest,
  AddAgentRequest,
  UpdateAgentRequest,
  RemoveAgentRequest,
  AddAppRequest,
  UpdateAppRequest,
  RemoveAppRequest
} from './generated/types.gen.js';