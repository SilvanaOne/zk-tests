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
  FileText,
} from "lucide-react";
import { useUserState } from "@/context/userState";
import { getLoopHoldings, getLoopActiveContracts, signLoopMessage, verifyLoopSignature, getLoopPublicKey, verifyPartyIdMatchesPublicKey, type ActiveContract } from "@/lib/loop";
import type { Holding } from "@fivenorth/loop-sdk";

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

  // Active Contracts state
  const [activeContracts, setActiveContracts] = useState<ActiveContract[]>([]);
  const [contractsLoading, setContractsLoading] = useState(false);
  const [contractsError, setContractsError] = useState<string | null>(null);

  // PartyId verification state
  const [partyIdVerified, setPartyIdVerified] = useState<boolean | null>(null);

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

  // Fetch active contracts when connected (only Amulet contracts)
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
        console.log("[LoopWallet] First contract full structure:", JSON.stringify(result[0], null, 2));
        console.log("[LoopWallet] contractEntry:", result[0].contractEntry);
        console.log("[LoopWallet] JsActiveContract:", result[0].contractEntry?.JsActiveContract);
        console.log("[LoopWallet] createdEvent:", result[0].contractEntry?.JsActiveContract?.createdEvent);
      }
      if (result) {
        setActiveContracts(result);
      } else {
        setActiveContracts([]);
      }
    } catch (error: any) {
      console.error("[LoopWallet] Failed to fetch active contracts:", error);
      setContractsError(error?.message || "Failed to fetch contracts");
    } finally {
      setContractsLoading(false);
    }
  }, [isConnected]);

  useEffect(() => {
    if (isConnected) {
      fetchHoldings();
      fetchActiveContracts();
    }
  }, [isConnected, fetchHoldings, fetchActiveContracts]);

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

        {/* Active Contracts Section */}
        <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
          <div className="flex items-center justify-between">
            <h4 className="text-sm font-semibold text-foreground flex items-center">
              <FileText className="w-4 h-4 mr-2" />
              Active Contracts
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
              <p className="text-xs text-muted-foreground">Loading contracts (may take up to 20s)...</p>
            </div>
          )}

          {contractsError && (
            <Alert variant="destructive" className="p-2">
              <XCircle className="h-4 w-4" />
              <AlertDescription className="text-xs">{contractsError}</AlertDescription>
            </Alert>
          )}

          {!contractsLoading && !contractsError && activeContracts.length === 0 && (
            <p className="text-xs text-muted-foreground text-center py-2">
              No active contracts found
            </p>
          )}

          {!contractsLoading && !contractsError && activeContracts.length > 0 && (
            <div className="space-y-2">
              {activeContracts.map((contract, index) => {
                // Handle nested JsActiveContract.createdEvent structure from API
                // API returns: { contractEntry: { JsActiveContract: { createdEvent: { templateId, contractId, ... } } } }
                const createdEvent = contract.contractEntry?.JsActiveContract?.createdEvent;
                const templateId = createdEvent?.templateId || createdEvent?.template_id || contract.template_id;
                const contractId = createdEvent?.contractId || createdEvent?.contract_id || contract.contract_id;
                const packageName = createdEvent?.packageName || createdEvent?.package_name;
                const createdAt = createdEvent?.createdAt;

                return (
                  <div
                    key={`${contractId}-${index}`}
                    className="p-2 rounded-md bg-background/50 border border-border/50"
                  >
                    <div className="space-y-1">
                      {packageName && (
                        <div className="flex items-center gap-2">
                          <span className="text-xs text-muted-foreground">Package:</span>
                          <span className="text-xs font-medium">{packageName}</span>
                        </div>
                      )}
                      {templateId && (
                        <div className="flex items-center gap-2">
                          <span className="text-xs text-muted-foreground">Template:</span>
                          <span className="text-xs font-mono truncate flex-1" title={templateId}>
                            {templateId.split(":").slice(-2).join(":")}
                          </span>
                        </div>
                      )}
                      {contractId && (
                        <div className="flex items-center gap-2">
                          <span className="text-xs text-muted-foreground shrink-0">Contract:</span>
                          <span
                            className="text-xs font-mono break-all flex-1 cursor-pointer hover:text-primary"
                            title="Click to copy"
                            onClick={() => {
                              navigator.clipboard.writeText(contractId);
                            }}
                          >
                            {contractId}
                          </span>
                        </div>
                      )}
                      {createdAt && (
                        <div className="flex items-center gap-2">
                          <span className="text-xs text-muted-foreground">Created:</span>
                          <span className="text-xs">
                            {new Date(createdAt).toLocaleString()}
                          </span>
                        </div>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
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
