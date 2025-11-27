"use client";

import { useState, useEffect } from "react";
import { StatusCard, DataRow, StatusPill } from "./status-card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  Activity,
  Send,
  Download,
  Edit3,
  CheckCircle,
  XCircle,
  ExternalLink,
  Loader2,
  Wallet,
  RefreshCw,
} from "lucide-react";
import { useUserState } from "@/context/userState";
import {
  balance as getBalance,
  faucet,
  preparePayment,
  broadcastPayment,
  explorerUrl,
} from "@/lib/mina";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Label } from "@/components/ui/label";

export function MinaWalletDashboard() {
  const { state: userState, apiFunctions } = useUserState();

  const publicKey = userState.selectedAuthMethod?.minaPublicKey;
  const isConnected = !!userState.selectedAuthMethod?.minaPublicKey;
  const [messageToSign, setMessageToSign] = useState("Hello Loop world!");
  const [signedMessage, setSignedMessage] = useState<{
    message: string;
    signature: string;
  } | null>(null);
  const [transferRecipient, setTransferRecipient] = useState("");
  const [transferAmount, setTransferAmount] = useState("");
  const [balance, setBalance] = useState<string>("0.00");
  const [balanceLoading, setBalanceLoading] = useState(false);
  const [faucetStatus, setFaucetStatus] = useState<{
    status: "idle" | "loading" | "success" | "error";
    txHash?: string;
    url?: string;
    error?: string;
  }>({ status: "idle" });
  const [transferStatus, setTransferStatus] = useState<{
    status: "idle" | "loading" | "success" | "error";
    txHash?: string;
    url?: string;
    error?: string;
  }>({ status: "idle" });
  const [signStatus, setSignStatus] = useState<
    "idle" | "loading" | "success" | "error"
  >("idle");
  const [chain, setChain] = useState<"zeko" | "devnet">("zeko");

  useEffect(() => {
    const fetchBalance = async (showLoading = true) => {
      if (isConnected && publicKey) {
        if (showLoading) {
          setBalanceLoading(true);
        }
        const balanceAmount = await getBalance({
          address: publicKey,
          chain,
        });
        setBalance((BigInt(balanceAmount) / 1_000_000n / 1000n).toString());
        if (showLoading) {
          setBalanceLoading(false);
        }
      } else {
        setBalance("0.00");
      }
    };
    fetchBalance();

    // Set up interval to refresh balance every 30 seconds
    const interval = setInterval(
      () => {
        if (isConnected && publicKey) {
          fetchBalance(false);
        }
      },
      chain === "zeko" ? 5000 : 30000
    );

    // Cleanup interval on unmount or dependency change
    return () => clearInterval(interval);
  }, [isConnected, publicKey, chain]);

  const refreshBalance = async () => {
    if (isConnected && publicKey) {
      setBalanceLoading(true);
      const balanceAmount = await getBalance({
        address: publicKey,
        chain,
      });
      setBalance((BigInt(balanceAmount) / 1_000_000n / 1000n).toString());
    } else {
      setBalance("0.00");
    }
    setBalanceLoading(false);
  };

  const handleSignMessage = async () => {
    if (!messageToSign || !publicKey) return;
    setSignStatus("loading");
    const signature = await apiFunctions.signMessage({
      publicKey,
      message: messageToSign,
    });
    if (signature.signature) {
      setSignedMessage({
        message: messageToSign,
        signature: signature.signature,
      });
    } else {
      setSignStatus("error");
    }
    setSignStatus("success");
    setMessageToSign("");
  };

  const handleFaucet = async () => {
    if (!publicKey) return;
    setFaucetStatus({ status: "loading" });
    const faucetResult = await faucet({
      address: publicKey,
      chain,
    });
    if (
      faucetResult &&
      faucetResult.success &&
      faucetResult.txHash &&
      typeof faucetResult.txHash === "string"
    ) {
      setFaucetStatus({
        status: "success",
        txHash: faucetResult.txHash,
        url: await explorerUrl({ chain, txHash: faucetResult.txHash }),
      });
    } else {
      setFaucetStatus({
        status: "error",
        error:
          ("error" in faucetResult && faucetResult.error) ||
          "Faucet request failed. Please try again later.",
      });
    }
  };

  const handleTransfer = async () => {
    if (!transferRecipient || !transferAmount || !publicKey) return;
    const amount = Number(transferAmount);
    if (isNaN(amount)) {
      setTransferStatus({
        status: "error",
        error: "Invalid amount. Please enter a valid number.",
      });
      return;
    }
    setTransferStatus({ status: "loading" });
    const payment = await preparePayment({
      from: publicKey,
      to: transferRecipient,
      amount: BigInt(amount * 1_000) * 1_000_000n,
      fee: BigInt(100_000_000n),
      memo: "Silvana Loop transfer",
      chain,
    });
    if (!payment) {
      setTransferStatus({
        status: "error",
        error: "Transfer failed - cannot get nonce. Try again later.",
      });
      return;
    }
    const signedPayment = await apiFunctions.signPayment({
      payment,
      publicKey,
    });
    if (!signedPayment.signature) {
      setTransferStatus({
        status: "error",
        error:
          signedPayment.error ||
          "Transfer failed - cannot sign payment. Try again later.",
      });
      return;
    }
    const broadcastResult = await broadcastPayment({
      payment: signedPayment.signature,
      chain,
    });
    if (
      !broadcastResult ||
      broadcastResult?.success === false ||
      !broadcastResult.txHash
    ) {
      setTransferStatus({
        status: "error",
        error:
          ("error" in broadcastResult && broadcastResult.error) ||
          "Transfer failed - cannot broadcast payment. Try again later when previous transactions will be included in the block.",
      });
      return;
    }
    setTransferStatus({
      status: "success",
      txHash: broadcastResult.txHash,
      url: await explorerUrl({ chain, txHash: broadcastResult.txHash }),
    });
    setTransferRecipient("");
    setTransferAmount("");
  };

  if (!isConnected) {
    return (
      <StatusCard
        title={chain === "zeko" ? "Loop Wallet" : "Mina Wallet"}
        icon={Activity}
        description={
          chain === "zeko"
            ? "Connect a wallet to manage your Loop assets."
            : "Connect a wallet to manage your Mina assets."
        }
        className="h-full"
      >
        <div className="text-center py-4">
          <StatusPill
            status="info"
            text={
              chain === "zeko"
                ? "Loop Wallet Not Active"
                : "Mina Wallet Not Active"
            }
          />
          <p className="text-sm text-muted-foreground mt-2">
            Please connect a primary wallet to activate{" "}
            {chain === "zeko" ? "Loop" : "Mina"} functionalities.
          </p>
        </div>
      </StatusCard>
    );
  }

  return (
    <StatusCard
      title={
        chain === "zeko" ? "Loop Wallet" : "Mina Wallet"
      }
      icon={Activity}
      description={
        chain === "zeko"
          ? "Manage your Loop assets securely."
          : "Manage your Mina assets securely."
      }
      className="h-full"
    >
      <div className="space-y-4">
        <div className="flex items-center space-x-3">
          <span className="text-sm font-semibold text-foreground">Chain:</span>
          <RadioGroup
            value={chain}
            onValueChange={(value) => setChain(value as "zeko" | "devnet")}
            className="flex items-center space-x-4"
          >
            <div className="flex items-center space-x-1">
              <RadioGroupItem value="zeko" id="chain-zeko" />
              <Label htmlFor="chain-zeko" className="text-xs">
                Zeko Devnet
              </Label>
            </div>
            <div className="flex items-center space-x-1">
              <RadioGroupItem value="devnet" id="chain-devnet" />
              <Label htmlFor="chain-devnet" className="text-xs">
                Mina Devnet
              </Label>
            </div>
          </RadioGroup>
        </div>
        <DataRow
          label={chain === "zeko" ? "Zeko Public Key" : "Mina Public Key"}
          value={publicKey}
          truncate={false}
        />

        {/* Balance Section */}
        <div className="p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
          <div className="flex items-center justify-between">
            <div className="flex items-center space-x-2">
              <Wallet className="w-4 h-4 text-brand-blue" />
              <span className="text-sm font-semibold text-foreground">
                Account Balance
              </span>
            </div>
            <div className="flex items-center space-x-2">
              <button
                onClick={refreshBalance}
                className="p-1 hover:bg-white/10 rounded transition-colors"
                title="Refresh balance"
              >
                <RefreshCw className="w-3 h-3 text-brand-green" />
              </button>
              {balanceLoading ? (
                <Loader2 className="w-6 h-6 animate-spin" />
              ) : (
                <span className="text-base font-bold text-brand-green">
                  {balance} MINA
                </span>
              )}
            </div>
          </div>
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
              <AlertDescription className="space-y-1 text-xs">
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
              </AlertDescription>
            </Alert>
          )}
        </div>

        {/* Faucet Section */}
        <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
          <h4 className="text-sm font-semibold text-foreground flex items-center">
            <Download className="w-4 h-4 mr-2" />
            {chain === "zeko" ? "Zeko Faucet" : "Mina Faucet"}
          </h4>
          <Button
            onClick={handleFaucet}
            disabled={faucetStatus.status === "loading"}
            className="w-full bg-brand-purple hover:bg-brand-purple/90 text-white h-8 text-xs"
          >
            {faucetStatus.status === "loading" && (
              <Loader2 className="mr-2 h-3 w-3 animate-spin" />
            )}
            Request Test Mina
          </Button>
          {faucetStatus.status === "success" && faucetStatus.txHash && (
            <Alert
              variant="default"
              className="mt-2 p-2 bg-muted/30 border-border"
            >
              <CheckCircle className="h-4 w-4 text-brand-green" />
              <AlertTitle className="text-sm text-foreground">
                Faucet Request Successful
              </AlertTitle>
              <AlertDescription className="text-xs">
                View Transaction
                <a
                  href={faucetStatus.url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="underline hover:text-primary flex items-center"
                >
                  {faucetStatus.txHash}
                  <ExternalLink className="h-3 w-3 ml-1" />
                </a>
              </AlertDescription>
            </Alert>
          )}
          {faucetStatus.status === "error" && (
            <Alert variant="destructive" className="mt-2 p-2">
              <XCircle className="h-4 w-4" />
              <AlertTitle className="text-sm">Error</AlertTitle>
              <AlertDescription className="text-xs">
                {faucetStatus.error}
              </AlertDescription>
            </Alert>
          )}
        </div>

        {/* Transfer Section */}
        <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
          <h4 className="text-sm font-semibold text-foreground flex items-center">
            <Send className="w-4 h-4 mr-2" />
            Transfer Mina
          </h4>
          <Input
            placeholder="Recipient Address (B62...)"
            value={transferRecipient}
            onChange={(e) => setTransferRecipient(e.target.value)}
            className="bg-input border-border focus:border-primary h-8 text-xs text-foreground placeholder:text-muted-foreground"
          />
          <Input
            type="number"
            placeholder="Amount (MINA)"
            value={transferAmount}
            onChange={(e) => setTransferAmount(e.target.value)}
            className="bg-input border-border focus:border-primary h-8 text-xs text-foreground placeholder:text-muted-foreground"
          />
          <Button
            onClick={handleTransfer}
            disabled={
              !transferRecipient ||
              !transferAmount ||
              transferStatus.status === "loading"
            }
            className="w-full bg-brand-blue hover:bg-brand-blue/90 text-white h-8 text-xs"
          >
            {transferStatus.status === "loading" && (
              <Loader2 className="mr-2 h-3 w-3 animate-spin" />
            )}
            Transfer Mina
          </Button>
          {transferStatus.status === "success" && transferStatus.txHash && (
            <Alert
              variant="default"
              className="mt-2 p-2 bg-muted/30 border-border"
            >
              <CheckCircle className="h-4 w-4 text-brand-green" />
              <AlertTitle className="text-sm text-foreground">
                Transfer Successful
              </AlertTitle>
              <AlertDescription className="text-xs">
                View Transaction
                <a
                  href={transferStatus.url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="underline hover:text-primary flex items-center"
                >
                  {transferStatus.txHash}
                  <ExternalLink className="h-3 w-3 ml-1" />
                </a>
              </AlertDescription>
            </Alert>
          )}
          {transferStatus.status === "error" && (
            <Alert variant="destructive" className="mt-2 p-2">
              <XCircle className="h-4 w-4" />
              <AlertTitle className="text-sm">Error</AlertTitle>
              <AlertDescription className="text-xs">
                {transferStatus.error}
              </AlertDescription>
            </Alert>
          )}
        </div>
      </div>
    </StatusCard>
  );
}
