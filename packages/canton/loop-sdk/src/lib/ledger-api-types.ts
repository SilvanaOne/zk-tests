// Types for Ledger API responses - exported separately from server actions

export interface LedgerHolding {
  contractId: string;
  templateId: string;
  tokenId: string;
  amount: string;
  isLocked: boolean;
  lockInfo?: {
    holders?: string[];
    expiresAt?: string;
    context?: string;
  };
}

export interface LedgerActiveContract {
  contractId: string;
  templateId: string;
  createArgument: any;
  createdEventBlob?: string;
}
