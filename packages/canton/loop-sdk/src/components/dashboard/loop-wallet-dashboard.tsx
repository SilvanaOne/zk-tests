"use client";

import { useState, useEffect, useCallback } from "react";
import { StatusCard, DataRow, StatusPill } from "./status-card";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  Activity,
  Edit3,
  CheckCircle,
  XCircle,
  Loader2,
  Coins,
  RefreshCw,
  ShieldCheck,
  Send,
  UserCheck,
  ExternalLink,
  Download,
} from "lucide-react";
import { Input } from "@/components/ui/input";
import { useUserState } from "@/context/userState";
import { getLoopHoldings, getLoopActiveContracts, signLoopMessage, verifyLoopSignature, getLoopPublicKey, verifyPartyIdMatchesPublicKey, transferCC, createTransferPreapprovalProposal, type TransferResult, type PreapprovalResult } from "@/lib/loop";
import { fetchContractDetails, type TransferPreapprovalCreateArguments, type AmuletCreateArguments, type ContractDetails } from "@/lib/blob";
import type { Holding } from "@fivenorth/loop-sdk";

// Type for preapproval contract with decoded details
interface PreapprovalContract {
  contractId: string;
  templateId: string;
  provider: string;
  receiver: string;
  expiresAt?: string;
  createdAt?: string;
}

// Type for Amulet contract with decoded details from Lighthouse API
interface AmuletContract {
  contractId: string;
  templateId: string;
  packageName: string;
  owner: string;
  dso: string;
  amount: AmuletCreateArguments["amount"];
  createdAt?: string;
  createdEventBlob: string;  // Base64-encoded protobuf blob for disclosed contracts
  blob: ContractDetails | null;  // Decoded contract details from Lighthouse API
}

interface LoopWalletDashboardProps {
  loopPartyId?: string | null;
  network?: "devnet" | "testnet" | "mainnet";
}

export function LoopWalletDashboard({ loopPartyId, network = "devnet" }: LoopWalletDashboardProps) {
  const { state: userState } = useUserState();

  const publicKey = userState.selectedAuthMethod?.minaPublicKey;
  const isConnected = !!loopPartyId;
  const [messageToSign, setMessageToSign] = useState("Hello Loop world!");
  const [signedMessage, setSignedMessage] = useState<{
    message: string;
    signature: string;
  } | null>(null);
  const [signStatus, setSignStatus] = useState<
    "idle" | "loading" | "success" | "error"
  >("idle");
  const [verifyStatus, setVerifyStatus] = useState<
    "idle" | "loading" | "valid" | "invalid"
  >("idle");

  // Holdings state
  const [holdings, setHoldings] = useState<Holding[]>([]);
  const [holdingsLoading, setHoldingsLoading] = useState(false);
  const [holdingsError, setHoldingsError] = useState<string | null>(null);

  // Active Contracts state (enriched with Lighthouse API data)
  const [amuletContracts, setAmuletContracts] = useState<AmuletContract[]>([]);
  const [contractsLoading, setContractsLoading] = useState(false);
  const [contractsError, setContractsError] = useState<string | null>(null);

  // PartyId verification state
  const [partyIdVerified, setPartyIdVerified] = useState<boolean | null>(null);

  // Transfer state
  const [transferReceiver, setTransferReceiver] = useState("");
  const [transferAmount, setTransferAmount] = useState("");
  const [transferDescription, setTransferDescription] = useState("");
  const [transferStatus, setTransferStatus] = useState<"idle" | "awaiting" | "loading" | "success" | "error">("idle");
  const [transferResult, setTransferResult] = useState<TransferResult | null>(null);

  // Preapproval state
  const [preapprovalProvider, setPreapprovalProvider] = useState("");
  const [preapprovalStatus, setPreapprovalStatus] = useState<"idle" | "awaiting" | "loading" | "success" | "error">("idle");
  const [preapprovalResult, setPreapprovalResult] = useState<PreapprovalResult | null>(null);

  // Preapproval contracts state (from Loop SDK)
  const [acceptedPreapprovals, setAcceptedPreapprovals] = useState<PreapprovalContract[]>([]);
  const [preapprovalContractsLoading, setPreapprovalContractsLoading] = useState(false);

  // Capitalize first letter for display
  const networkDisplay = network.charAt(0).toUpperCase() + network.slice(1);

  // Fetch holdings when connected
  const fetchHoldings = useCallback(async () => {
    console.log("[LoopWallet] fetchHoldings called, isConnected:", isConnected);
    if (!isConnected) return;

    setHoldingsLoading(true);
    setHoldingsError(null);
    try {
      const result = await getLoopHoldings();
      console.log("[LoopWallet] Holdings result:", result);
      if (result) {
        setHoldings(result);
      } else {
        setHoldings([]);
      }
    } catch (error: any) {
      console.error("[LoopWallet] Failed to fetch holdings:", error);
      setHoldingsError(error?.message || "Failed to fetch holdings");
    } finally {
      setHoldingsLoading(false);
    }
  }, [isConnected]);

  // Fetch active contracts when connected (Amulet contracts enriched with Lighthouse API)
  const fetchActiveContracts = useCallback(async () => {
    console.log("[LoopWallet] fetchActiveContracts called, isConnected:", isConnected);
    if (!isConnected) return;

    setContractsLoading(true);
    setContractsError(null);
    try {
      const result = await getLoopActiveContracts({
        templateId: "#splice-amulet:Splice.Amulet:Amulet"
      });
      console.log("[LoopWallet] Active contracts result:", result);

      if (result && result.length > 0) {
        // Fetch details from Lighthouse API for each contract
        const enrichedContracts: AmuletContract[] = await Promise.all(
          result.map(async (contract) => {
            const createdEvent = contract.contractEntry?.JsActiveContract?.createdEvent;
            const contractId = createdEvent?.contractId || createdEvent?.contract_id || "";

            // Fetch full details from Lighthouse API
            const details = await fetchContractDetails(contractId, network);
            const args = details?.create_arguments as AmuletCreateArguments | undefined;

            return {
              contractId,
              templateId: details?.template_id || createdEvent?.templateId || "",
              packageName: details?.package_name || createdEvent?.packageName || "",
              owner: args?.owner || "",
              dso: args?.dso || "",
              amount: args?.amount || { initialAmount: "0", createdAt: { number: "0" }, ratePerRound: { rate: "0" } },
              createdAt: createdEvent?.createdAt,
              createdEventBlob: createdEvent?.createdEventBlob || "",
              blob: details,
            };
          })
        );

        console.log("[LoopWallet] Enriched Amulet contracts:", enrichedContracts);
        setAmuletContracts(enrichedContracts);
      } else {
        setAmuletContracts([]);
      }
    } catch (error: any) {
      console.error("[LoopWallet] Failed to fetch active contracts:", error);
      setContractsError(error?.message || "Failed to fetch contracts");
    } finally {
      setContractsLoading(false);
    }
  }, [isConnected, network]);

  // Fetch preapproval contracts from Loop SDK and enrich with Lighthouse API
  const fetchPreapprovalContracts = useCallback(async () => {
    console.log("[LoopWallet] fetchPreapprovalContracts called, isConnected:", isConnected, "partyId:", loopPartyId);
    if (!isConnected || !loopPartyId) return;

    setPreapprovalContractsLoading(true);
    try {
      // Fetch TransferPreapproval contracts directly from Loop SDK
      // This gives us all preapprovals where user is receiver (has visibility)
      const contracts = await getLoopActiveContracts({
        templateId: "#splice-amulet:Splice.AmuletRules:TransferPreapproval"
      });

      console.log("[LoopWallet] TransferPreapproval contracts from Loop SDK:", contracts);

      if (contracts && contracts.length > 0) {
        // Extract contract IDs and fetch details from Lighthouse API
        const preapprovals: PreapprovalContract[] = await Promise.all(
          contracts.map(async (contract) => {
            const createdEvent = contract.contractEntry?.JsActiveContract?.createdEvent;
            const contractId = createdEvent?.contractId || createdEvent?.contract_id || "";

            // Fetch full details from Lighthouse API
            const details = await fetchContractDetails(contractId, network);
            const args = details?.create_arguments as TransferPreapprovalCreateArguments | undefined;

            return {
              contractId,
              templateId: createdEvent?.templateId || createdEvent?.template_id || contract.template_id,
              provider: args?.provider || "",
              receiver: args?.receiver || loopPartyId,
              expiresAt: args?.expiresAt,
              createdAt: createdEvent?.createdAt,
            };
          })
        );

        console.log("[LoopWallet] Mapped preapprovals with Lighthouse details:", preapprovals);
        setAcceptedPreapprovals(preapprovals);
      } else {
        console.log("[LoopWallet] No TransferPreapproval contracts found");
        setAcceptedPreapprovals([]);
      }
    } catch (error: any) {
      console.error("[LoopWallet] Failed to fetch preapproval contracts:", error);
      setAcceptedPreapprovals([]);
    } finally {
      setPreapprovalContractsLoading(false);
    }
  }, [isConnected, loopPartyId, network]);

  useEffect(() => {
    if (isConnected) {
      fetchHoldings();
      fetchActiveContracts();
      fetchPreapprovalContracts();
    }
  }, [isConnected, fetchHoldings, fetchActiveContracts, fetchPreapprovalContracts]);

  // Verify partyId matches public key when connected
  useEffect(() => {
    const verifyPartyId = async () => {
      const publicKey = getLoopPublicKey();
      if (loopPartyId && publicKey) {
        const isValid = await verifyPartyIdMatchesPublicKey(loopPartyId, publicKey);
        setPartyIdVerified(isValid);
      } else {
        setPartyIdVerified(null);
      }
    };
    verifyPartyId();
  }, [loopPartyId]);

  // Wallet URLs by network
  const walletUrls: Record<string, string> = {
    devnet: "https://devnet.cantonloop.com",
    testnet: "https://testnet.cantonloop.com",
    mainnet: "https://cantonloop.com",
  };

  // Open Loop wallet in new tab (or focus if already open with same name)
  const openLoopWallet = () => {
    window.open(walletUrls[network] || walletUrls.devnet, "loop-wallet");
  };

  // Format balance - the API returns decimal strings like "10000.000000"
  const formatBalance = (amount: string): string => {
    const num = parseFloat(amount);
    if (isNaN(num)) return "0";
    // Format with up to 4 decimal places, removing trailing zeros
    return num.toLocaleString(undefined, {
      minimumFractionDigits: 0,
      maximumFractionDigits: 4,
    });
  };

  // Download contract data as JSON file for use as disclosed contract in transactions
  const downloadContractJson = (contract: AmuletContract) => {
    const data = {
      templateId: contract.templateId,
      contractId: contract.contractId,
      createdEventBlob: contract.createdEventBlob,  // Base64-encoded protobuf for disclosed contracts
      create_arguments: contract.blob?.create_arguments,  // Decoded payload for reference
    };
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `contract-${contract.contractId.slice(0, 8)}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleSignMessage = async () => {
    if (!messageToSign || !isConnected) return;
    setSignStatus("loading");
    try {
      const result = await signLoopMessage(messageToSign);
      console.log("[LoopWallet] Sign message result:", result);
      if (result) {
        setSignedMessage({
          message: messageToSign,
          signature: typeof result === "string" ? result : JSON.stringify(result),
        });
        setSignStatus("success");
      } else {
        setSignStatus("error");
      }
    } catch (error: any) {
      console.error("[LoopWallet] Sign message error:", error);
      setSignStatus("error");
    }
    setMessageToSign("");
    setVerifyStatus("idle");
  };

  const handleVerifySignature = async () => {
    if (!signedMessage) return;
    setVerifyStatus("loading");
    try {
      const isValid = await verifyLoopSignature(
        signedMessage.message,
        signedMessage.signature
      );
      setVerifyStatus(isValid ? "valid" : "invalid");
    } catch (error) {
      console.error("[LoopWallet] Verification error:", error);
      setVerifyStatus("invalid");
    }
  };

  const handleTransfer = async () => {
    if (!transferReceiver || !transferAmount || !isConnected) return;

    setTransferStatus("awaiting");
    setTransferResult(null);

    try {
      const result = await transferCC({
        receiver: transferReceiver,
        amount: transferAmount,
        description: transferDescription || undefined,
      });

      setTransferResult(result);
      setTransferStatus(result.success ? "success" : "error");

      if (result.success) {
        // Clear form on success
        setTransferReceiver("");
        setTransferAmount("");
        setTransferDescription("");
        // Refresh holdings after successful transfer
        fetchHoldings();
        fetchActiveContracts();
      }
    } catch (error: any) {
      console.error("[LoopWallet] Transfer error:", error);
      setTransferResult({ success: false, error: error?.message || "Transfer failed" });
      setTransferStatus("error");
    }
  };

  const handleCreatePreapproval = async () => {
    if (!isConnected || !preapprovalProvider.trim()) return;

    setPreapprovalStatus("awaiting");
    setPreapprovalResult(null);

    try {
      const result = await createTransferPreapprovalProposal({
        provider: preapprovalProvider.trim()
      });
      setPreapprovalResult(result);
      setPreapprovalStatus(result.success ? "success" : "error");

      if (result.success) {
        // Clear input and refresh preapproval contracts after successful creation
        setPreapprovalProvider("");
        fetchPreapprovalContracts();
      }
    } catch (error: any) {
      console.error("[LoopWallet] Preapproval error:", error);
      setPreapprovalResult({ success: false, error: error?.message || "Failed to create preapproval" });
      setPreapprovalStatus("error");
    }
  };

  if (!isConnected) {
    return (
      <StatusCard
        title="Loop Wallet"
        icon={Activity}
        description="Connect a wallet to manage your Loop assets."
        className="h-full"
      >
        <div className="text-center py-4">
          <StatusPill
            status="info"
            text="Loop Wallet Not Active"
          />
          <p className="text-sm text-muted-foreground mt-2">
            Please connect to activate Loop functionalities.
          </p>
        </div>
      </StatusCard>
    );
  }

  return (
    <StatusCard
      title="Loop Wallet"
      icon={Activity}
      description="Manage your Loop assets securely."
      className="h-full"
    >
      <div className="space-y-4">
        <DataRow
          label="Network"
          value={networkDisplay}
          truncate={false}
        />
        {loopPartyId && (
          <div className="flex items-center gap-2">
            <DataRow
              label="Party ID"
              value={loopPartyId}
              truncate={false}
              className="flex-1"
            />
            {partyIdVerified === true && (
              <span title="Verified: Public key matches Party ID">
                <CheckCircle className="h-4 w-4 text-brand-green flex-shrink-0" />
              </span>
            )}
            {partyIdVerified === false && (
              <span title="Verification failed">
                <XCircle className="h-4 w-4 text-destructive flex-shrink-0" />
              </span>
            )}
          </div>
        )}
        {getLoopPublicKey() && (
          <div className="flex items-center gap-2">
            <DataRow
              label="Public Key"
              value={getLoopPublicKey()!}
              truncate={false}
              className="flex-1"
            />
            {partyIdVerified === true && (
              <span title="Verified: Public key matches Party ID">
                <CheckCircle className="h-4 w-4 text-brand-green flex-shrink-0" />
              </span>
            )}
            {partyIdVerified === false && (
              <span title="Verification failed">
                <XCircle className="h-4 w-4 text-destructive flex-shrink-0" />
              </span>
            )}
          </div>
        )}

        {/* Holdings Section */}
        <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
          <div className="flex items-center justify-between">
            <h4 className="text-sm font-semibold text-foreground flex items-center">
              <Coins className="w-4 h-4 mr-2" />
              Holdings
            </h4>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => fetchHoldings()}
              disabled={holdingsLoading}
              className="h-6 w-6 p-0"
            >
              <RefreshCw className={`h-3 w-3 ${holdingsLoading ? "animate-spin" : ""}`} />
            </Button>
          </div>

          {holdingsLoading && (
            <div className="flex items-center justify-center py-4">
              <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
            </div>
          )}

          {holdingsError && (
            <Alert variant="destructive" className="p-2">
              <XCircle className="h-4 w-4" />
              <AlertDescription className="text-xs">{holdingsError}</AlertDescription>
            </Alert>
          )}

          {!holdingsLoading && !holdingsError && holdings.length === 0 && (
            <p className="text-xs text-muted-foreground text-center py-2">
              No holdings found
            </p>
          )}

          {!holdingsLoading && !holdingsError && holdings.length > 0 && (
            <div className="space-y-2">
              {holdings.map((holding, index) => (
                <div
                  key={`${holding.instrument_id.admin}-${holding.instrument_id.id}-${index}`}
                  className="flex items-center gap-3 p-2 rounded-md bg-background/50 border border-border/50"
                >
                  {holding.image && (
                    <img
                      src={holding.image}
                      alt={holding.symbol}
                      className="w-8 h-8 rounded-full"
                      onError={(e) => {
                        e.currentTarget.style.display = "none";
                      }}
                    />
                  )}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="font-medium text-sm">{holding.symbol}</span>
                      <span className="text-xs text-muted-foreground truncate">
                        {holding.org_name}
                      </span>
                    </div>
                    <div className="text-xs text-muted-foreground">
                      <span className="text-foreground font-medium">
                        {formatBalance(holding.total_unlocked_coin)}
                      </span>
                      {parseFloat(holding.total_locked_coin) > 0 && (
                        <span className="ml-2 text-brand-yellow">
                          ({formatBalance(holding.total_locked_coin)} locked)
                        </span>
                      )}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Canton Coin Holdings Section */}
        <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
          <div className="flex items-center justify-between">
            <h4 className="text-sm font-semibold text-foreground flex items-center">
              <Coins className="w-4 h-4 mr-2" />
              Canton Coin Holdings
            </h4>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => fetchActiveContracts()}
              disabled={contractsLoading}
              className="h-6 w-6 p-0"
            >
              <RefreshCw className={`h-3 w-3 ${contractsLoading ? "animate-spin" : ""}`} />
            </Button>
          </div>

          {contractsLoading && (
            <div className="flex flex-col items-center justify-center py-4 gap-2">
              <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
              <p className="text-xs text-muted-foreground">Loading holdings...</p>
            </div>
          )}

          {contractsError && (
            <Alert variant="destructive" className="p-2">
              <XCircle className="h-4 w-4" />
              <AlertDescription className="text-xs">{contractsError}</AlertDescription>
            </Alert>
          )}

          {!contractsLoading && !contractsError && amuletContracts.length === 0 && (
            <p className="text-xs text-muted-foreground text-center py-2">
              No Canton Coin holdings found
            </p>
          )}

          {!contractsLoading && !contractsError && amuletContracts.length > 0 && (
            <div className="space-y-2">
              {amuletContracts.map((contract, index) => (
                <div
                  key={`${contract.contractId}-${index}`}
                  className="p-2 rounded-md bg-background/50 border border-border/50"
                >
                  <div className="space-y-1">
                    {/* Amount prominently displayed with download button */}
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2">
                        <Coins className="w-4 h-4 text-brand-yellow" />
                        <span className="text-sm font-semibold text-foreground">
                          {formatBalance(contract.amount.initialAmount)} CC
                        </span>
                      </div>
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        onClick={() => downloadContractJson(contract)}
                        className="h-6 w-6 p-0"
                        title="Download contract JSON"
                      >
                        <Download className="h-3 w-3" />
                      </Button>
                    </div>
                    {contract.packageName && (
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-muted-foreground">Package:</span>
                        <span className="text-xs font-medium">{contract.packageName}</span>
                      </div>
                    )}
                    {contract.templateId && (
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-muted-foreground">Template:</span>
                        <span className="text-xs font-mono truncate flex-1" title={contract.templateId}>
                          {contract.templateId.split(":").slice(-2).join(":")}
                        </span>
                      </div>
                    )}
                    {contract.contractId && (
                      <div className="flex items-start gap-2">
                        <span className="text-xs text-muted-foreground shrink-0">Contract:</span>
                        <span
                          className="text-xs font-mono break-all flex-1 cursor-pointer hover:text-primary"
                          title="Click to copy"
                          onClick={() => navigator.clipboard.writeText(contract.contractId)}
                        >
                          {contract.contractId}
                        </span>
                      </div>
                    )}
                    {contract.createdAt && (
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-muted-foreground">Created:</span>
                        <span className="text-xs">
                          {new Date(contract.createdAt).toLocaleString()}
                        </span>
                      </div>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Preapproval Contracts Section */}
        <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
          <div className="flex items-center justify-between">
            <h4 className="text-sm font-semibold text-foreground flex items-center">
              <UserCheck className="w-4 h-4 mr-2" />
              Preapproval Status
            </h4>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => fetchPreapprovalContracts()}
              disabled={preapprovalContractsLoading}
              className="h-6 w-6 p-0"
            >
              <RefreshCw className={`h-3 w-3 ${preapprovalContractsLoading ? "animate-spin" : ""}`} />
            </Button>
          </div>

          {preapprovalContractsLoading && (
            <div className="flex flex-col items-center justify-center py-4 gap-2">
              <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
              <p className="text-xs text-muted-foreground">Loading preapproval status...</p>
            </div>
          )}

          {!preapprovalContractsLoading && acceptedPreapprovals.length === 0 && (
            <p className="text-xs text-muted-foreground text-center py-2">
              No active preapproval found. Create a proposal below.
            </p>
          )}

          {/* Active Preapprovals */}
          {!preapprovalContractsLoading && acceptedPreapprovals.length > 0 && (
            <div className="space-y-2">
              {acceptedPreapprovals.map((preapproval, index) => (
                <div key={preapproval.contractId || index} className="p-2 rounded-md bg-brand-green/10 border border-brand-green/30">
                  <div className="space-y-1">
                    <div className="flex items-center gap-2">
                      <span className="text-xs text-muted-foreground">Status:</span>
                      <span className="text-xs font-medium text-brand-green">Active</span>
                    </div>
                    {preapproval.provider && (
                      <div className="flex items-start gap-2">
                        <span className="text-xs text-muted-foreground shrink-0">Provider:</span>
                        <span
                          className="text-xs font-mono break-all flex-1 cursor-pointer hover:text-primary"
                          title="Click to copy"
                          onClick={() => navigator.clipboard.writeText(preapproval.provider)}
                        >
                          {preapproval.provider}
                        </span>
                      </div>
                    )}
                    {preapproval.expiresAt && (
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-muted-foreground">Expires:</span>
                        <span className="text-xs">
                          {new Date(preapproval.expiresAt).toLocaleDateString()}
                        </span>
                      </div>
                    )}
                    {preapproval.contractId && (
                      <div className="flex items-start gap-2">
                        <span className="text-xs text-muted-foreground shrink-0">Contract:</span>
                        <span
                          className="text-xs font-mono break-all flex-1 cursor-pointer hover:text-primary"
                          title="Click to copy"
                          onClick={() => navigator.clipboard.writeText(preapproval.contractId)}
                        >
                          {preapproval.contractId}
                        </span>
                      </div>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Transfer CC Section */}
        <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
          <h4 className="text-sm font-semibold text-foreground flex items-center">
            <Send className="w-4 h-4 mr-2" />
            Transfer CC
          </h4>
          <div className="space-y-2">
            <div>
              <label className="text-xs text-muted-foreground">Receiver Party ID</label>
              <Input
                placeholder="Enter receiver's party ID..."
                value={transferReceiver}
                onChange={(e) => setTransferReceiver(e.target.value)}
                className="bg-input border-border focus:border-primary text-foreground text-xs placeholder:text-muted-foreground h-8 mt-1"
              />
            </div>
            <div>
              <label className="text-xs text-muted-foreground">Amount (CC)</label>
              <Input
                type="number"
                step="0.01"
                min="0"
                placeholder="0.00"
                value={transferAmount}
                onChange={(e) => setTransferAmount(e.target.value)}
                className="bg-input border-border focus:border-primary text-foreground text-xs placeholder:text-muted-foreground h-8 mt-1"
              />
            </div>
            <div>
              <label className="text-xs text-muted-foreground">Description (optional)</label>
              <Input
                placeholder="Transfer reason..."
                value={transferDescription}
                onChange={(e) => setTransferDescription(e.target.value)}
                className="bg-input border-border focus:border-primary text-foreground text-xs placeholder:text-muted-foreground h-8 mt-1"
              />
            </div>
          </div>
          <Button
            onClick={handleTransfer}
            disabled={!transferReceiver || !transferAmount || transferStatus === "loading" || transferStatus === "awaiting"}
            className="w-full bg-gradient-to-r from-brand-pink via-brand-purple to-brand-blue hover:brightness-105 text-white h-8 text-xs"
          >
            {transferStatus === "loading" && (
              <Loader2 className="mr-2 h-3 w-3 animate-spin" />
            )}
            <Send className="mr-2 h-3 w-3" />
            Send CC
          </Button>
          {transferStatus === "awaiting" && (
            <Alert className="mt-2 p-2 bg-brand-yellow/10 border-brand-yellow/30">
              <Loader2 className="h-4 w-4 animate-spin text-brand-yellow" />
              <AlertTitle className="text-sm text-foreground">Awaiting Approval</AlertTitle>
              <AlertDescription className="text-xs">
                <p className="text-muted-foreground mb-2">
                  Please approve this transaction in your Loop Wallet.
                </p>
                <Button
                  variant="link"
                  size="sm"
                  onClick={openLoopWallet}
                  className="h-6 p-0 text-xs text-brand-yellow hover:text-brand-yellow/80"
                >
                  Open Loop Wallet
                  <ExternalLink className="ml-1 h-3 w-3" />
                </Button>
              </AlertDescription>
            </Alert>
          )}
          {transferStatus === "error" && transferResult && (
            <Alert variant="destructive" className="mt-2 p-2">
              <XCircle className="h-4 w-4" />
              <AlertTitle className="text-sm">Transfer Failed</AlertTitle>
              <AlertDescription className="text-xs">
                {transferResult.error || "Unknown error occurred"}
              </AlertDescription>
            </Alert>
          )}
          {transferStatus === "success" && transferResult && (
            <Alert
              variant="default"
              className="mt-2 p-2 bg-muted/30 border-border"
            >
              <CheckCircle className="h-4 w-4 text-brand-green" />
              <AlertTitle className="text-sm text-foreground">
                Transfer Successful
              </AlertTitle>
              <AlertDescription className="space-y-1 text-xs">
                <p className="text-muted-foreground">
                  Your CC has been sent successfully.
                </p>
                {transferResult.updateId && (
                  <DataRow
                    label="Update ID"
                    value={transferResult.updateId}
                    truncate={true}
                    className="border-none py-0.5"
                    valueClassName="text-xs font-mono"
                  />
                )}
              </AlertDescription>
            </Alert>
          )}
        </div>

        {/* Preapprove Transfers Section */}
        <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
          <h4 className="text-sm font-semibold text-foreground flex items-center">
            <UserCheck className="w-4 h-4 mr-2" />
            Preapprove Transfers
          </h4>
          <p className="text-xs text-muted-foreground">
            Create a transfer preapproval proposal.
          </p>
          <div>
            <label className="text-xs text-muted-foreground">Provider Party ID</label>
            <Input
              placeholder="Enter provider's party ID..."
              value={preapprovalProvider}
              onChange={(e) => setPreapprovalProvider(e.target.value)}
              className="bg-input border-border focus:border-primary text-foreground text-xs placeholder:text-muted-foreground h-8 mt-1"
            />
          </div>
          <Button
            onClick={handleCreatePreapproval}
            disabled={!preapprovalProvider.trim() || preapprovalStatus === "loading" || preapprovalStatus === "awaiting"}
            className="w-full bg-gradient-to-r from-brand-pink via-brand-purple to-brand-blue hover:brightness-105 text-white h-8 text-xs"
          >
            {preapprovalStatus === "loading" && (
              <Loader2 className="mr-2 h-3 w-3 animate-spin" />
            )}
            <UserCheck className="mr-2 h-3 w-3" />
            Create Preapproval Proposal
          </Button>
          {preapprovalStatus === "awaiting" && (
            <Alert className="mt-2 p-2 bg-brand-yellow/10 border-brand-yellow/30">
              <Loader2 className="h-4 w-4 animate-spin text-brand-yellow" />
              <AlertTitle className="text-sm text-foreground">Awaiting Approval</AlertTitle>
              <AlertDescription className="text-xs">
                <p className="text-muted-foreground mb-2">
                  Please approve this transaction in your Loop Wallet.
                </p>
                <Button
                  variant="link"
                  size="sm"
                  onClick={openLoopWallet}
                  className="h-6 p-0 text-xs text-brand-yellow hover:text-brand-yellow/80"
                >
                  Open Loop Wallet
                  <ExternalLink className="ml-1 h-3 w-3" />
                </Button>
              </AlertDescription>
            </Alert>
          )}
          {preapprovalStatus === "error" && preapprovalResult && (
            <Alert variant="destructive" className="mt-2 p-2">
              <XCircle className="h-4 w-4" />
              <AlertTitle className="text-sm">Preapproval Failed</AlertTitle>
              <AlertDescription className="text-xs">
                {preapprovalResult.error || "Unknown error occurred"}
              </AlertDescription>
            </Alert>
          )}
          {preapprovalStatus === "success" && preapprovalResult && (
            <Alert
              variant="default"
              className="mt-2 p-2 bg-muted/30 border-border"
            >
              <CheckCircle className="h-4 w-4 text-brand-green" />
              <AlertTitle className="text-sm text-foreground">
                Proposal Created
              </AlertTitle>
              <AlertDescription className="space-y-1 text-xs">
                <p className="text-muted-foreground">
                  Transfer preapproval proposal has been created.
                  The provider needs to accept it to complete the preapproval.
                </p>
                {preapprovalResult.updateId && (
                  <DataRow
                    label="Update ID"
                    value={preapprovalResult.updateId}
                    truncate={true}
                    className="border-none py-0.5"
                    valueClassName="text-xs font-mono"
                  />
                )}
              </AlertDescription>
            </Alert>
          )}
        </div>

        {/* Sign Message Section */}
        <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
          <h4 className="text-sm font-semibold text-foreground flex items-center">
            <Edit3 className="w-4 h-4 mr-2" />
            Sign Message
          </h4>
          <Textarea
            placeholder="Enter message to sign..."
            value={messageToSign}
            onChange={(e) => setMessageToSign(e.target.value)}
            className="bg-input border-border focus:border-primary text-foreground text-xs placeholder:text-muted-foreground"
            rows={2}
          />
          <Button
            onClick={handleSignMessage}
            disabled={!messageToSign || signStatus === "loading"}
            className="w-full bg-gradient-to-r from-brand-pink via-brand-purple to-brand-blue hover:brightness-105 text-white h-8 text-xs"
          >
            {signStatus === "loading" && (
              <Loader2 className="mr-2 h-3 w-3 animate-spin" />
            )}
            Sign Message
          </Button>
          {signStatus === "error" && (
            <Alert variant="destructive" className="mt-2 p-2">
              <XCircle className="h-4 w-4" />
              <AlertTitle className="text-sm">Error</AlertTitle>
              <AlertDescription className="text-xs">
                Signing failed.
              </AlertDescription>
            </Alert>
          )}
          {signedMessage && signStatus === "success" && (
            <Alert
              variant="default"
              className="mt-2 p-2 bg-muted/30 border-border"
            >
              <CheckCircle className="h-4 w-4 text-brand-green" />
              <AlertTitle className="text-sm text-foreground">
                Message Signed
              </AlertTitle>
              <AlertDescription className="space-y-2 text-xs">
                <DataRow
                  label="Original Message"
                  value={signedMessage.message}
                  truncate={true}
                  className="border-none py-0.5"
                  valueClassName="text-xs"
                />
                <DataRow
                  label="Signature"
                  value={signedMessage.signature}
                  truncate={true}
                  className="border-none py-0.5"
                  valueClassName="text-xs"
                />
                <div className="flex items-center gap-2 pt-1">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={handleVerifySignature}
                    disabled={verifyStatus === "loading"}
                    className="h-7 text-xs"
                  >
                    {verifyStatus === "loading" ? (
                      <Loader2 className="mr-1 h-3 w-3 animate-spin" />
                    ) : (
                      <ShieldCheck className="mr-1 h-3 w-3" />
                    )}
                    Verify
                  </Button>
                  {verifyStatus === "valid" && (
                    <span className="flex items-center text-brand-green text-xs">
                      <CheckCircle className="h-3 w-3 mr-1" />
                      Valid
                    </span>
                  )}
                  {verifyStatus === "invalid" && (
                    <span className="flex items-center text-destructive text-xs">
                      <XCircle className="h-3 w-3 mr-1" />
                      Invalid
                    </span>
                  )}
                </div>
              </AlertDescription>
            </Alert>
          )}
        </div>

      </div>
    </StatusCard>
  );
}
