declare module "@fivenorth/loop-sdk" {
  export interface Holding {
    instrument_id: { admin: string; id: string };
    decimals: number;
    symbol: string;
    org_name: string;
    total_unlocked_coin: string;
    total_locked_coin: string;
    image: string;
  }

  export interface ActiveContract {
    template_id: string;
    contract_id: string;
    [key: string]: any;
  }

  export interface TransactionCommand {
    ExerciseCommand: {
      templateId: string;
      contractId: string;
      choice: string;
      choiceArgument: Record<string, any>;
    };
  }

  export interface TransactionPayload {
    commands: TransactionCommand[];
    disclosedContracts?: any[];
  }

  export interface LoopProvider {
    party_id: string;
    public_key: string;
    email?: string;
    getHolding(): Promise<Holding[]>;
    getActiveContracts(params?: { templateId?: string; interfaceId?: string }): Promise<ActiveContract[]>;
    signMessage(message: string): Promise<any>;
    submitTransaction(payload: TransactionPayload): Promise<any>;
  }

  export interface LoopInitOptions {
    appName: string;
    network: "devnet" | "testnet" | "mainnet";
    onAccept: (provider: LoopProvider) => void;
    onReject: () => void;
  }

  export interface Loop {
    init(options: LoopInitOptions): void;
    connect(): void;
  }

  export const loop: Loop;
}
