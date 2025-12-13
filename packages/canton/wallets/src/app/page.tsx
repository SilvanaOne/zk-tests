"use client";

import { useState } from "react";
import { LoopWalletDashboard } from "@/components/dashboard/loop-wallet-dashboard";
import { NetworkSelectModal, type NetworkType } from "@/components/dashboard/network-select-modal";
import { Button } from "@/components/ui/button";
import { AnimatedBackground } from "@/components/ui/animated-background";
import { useUserState } from "@/context/userState";
import { Globe, Wallet, LogOut, X } from "lucide-react";
import Image from "next/image";

export default function HomePage() {
  const [network, setNetwork] = useState<NetworkType>("devnet");
  const [showNetworkModal, setShowNetworkModal] = useState(false);
  const [showWalletModal, setShowWalletModal] = useState(false);

  const { connectLoop, connectPhantom, connectSolflare, getConnectedWalletInfo, disconnectWallet } = useUserState();

  const walletInfo = getConnectedWalletInfo();
  const isConnected = walletInfo.walletType !== null;

  const handleNetworkSelect = (selectedNetwork: NetworkType) => {
    setNetwork(selectedNetwork);
    setShowNetworkModal(false);
  };

  const handleConnectLoop = async () => {
    setShowWalletModal(false);
    await connectLoop(network);
  };

  const handleConnectPhantom = async () => {
    setShowWalletModal(false);
    await connectPhantom();
  };

  const handleConnectSolflare = async () => {
    setShowWalletModal(false);
    await connectSolflare();
  };

  const handleLogout = () => {
    disconnectWallet();
    window.location.reload();
  };

  // Truncate address for display
  const truncateAddress = (address: string, chars = 8) => {
    if (address.length <= chars * 2) return address;
    return `${address.slice(0, chars)}...${address.slice(-chars)}`;
  };

  return (
    <div className="min-h-screen relative">
      <AnimatedBackground />

      {/* Header */}
      <header className="relative z-10 border-b border-border/40 backdrop-blur-sm bg-background/80">
        <div className="container mx-auto px-4 py-4">
          <div className="flex items-center justify-between">
            <h1 className="text-2xl font-bold bg-gradient-to-r from-brand-pink via-brand-purple to-brand-blue text-transparent bg-clip-text">
              Silvana Wallet Connect
            </h1>
            <div className="flex items-center gap-3">
              <Button
                variant="outline"
                size="sm"
                onClick={() => setShowNetworkModal(true)}
                className="flex items-center gap-2"
              >
                <Globe className="h-4 w-4" />
                {network.charAt(0).toUpperCase() + network.slice(1)}
              </Button>

              {isConnected ? (
                <div className="flex items-center gap-2">
                  {/* Connected Wallet Info */}
                  <div className="flex items-center gap-2 px-3 py-1.5 rounded-md bg-muted/50 border border-border/50">
                    <Image
                      src={walletInfo.walletType === "loop" ? "/loop.png" : walletInfo.walletType === "phantom" ? "/phantom.svg" : "/solflare.svg"}
                      alt={walletInfo.walletName || ""}
                      width={20}
                      height={20}
                      className="rounded-sm"
                    />
                    <span className="text-sm font-medium">{walletInfo.walletName}</span>
                  </div>
                  <Button
                    onClick={handleLogout}
                    className="bg-gradient-to-r from-brand-pink via-brand-purple to-brand-blue hover:brightness-110 text-white"
                  >
                    <LogOut className="h-4 w-4 mr-2" />
                    Logout
                  </Button>
                </div>
              ) : (
                <Button
                  onClick={() => setShowWalletModal(true)}
                  className="bg-gradient-to-r from-brand-pink via-brand-purple to-brand-blue hover:brightness-110 text-white"
                >
                  <Wallet className="h-4 w-4 mr-2" />
                  Connect Wallet
                </Button>
              )}
            </div>
          </div>
        </div>
      </header>

      {/* Connected Wallet Details */}
      {isConnected && (
        <div className="relative z-10 border-b border-border/40 backdrop-blur-sm bg-muted/30">
          <div className="container mx-auto px-4 py-3">
            <div className="flex flex-wrap items-center gap-x-6 gap-y-2 text-sm">
              <div className="flex items-center gap-2">
                <span className="text-muted-foreground">Wallet:</span>
                <span className="font-medium">{walletInfo.walletName}</span>
              </div>
              {(walletInfo.walletType === "phantom" || walletInfo.walletType === "solflare") && walletInfo.solanaPublicKey && (
                <div className="flex items-center gap-2">
                  <span className="text-muted-foreground">Solana Public Key:</span>
                  <code className="font-mono text-xs bg-muted px-2 py-0.5 rounded">
                    {truncateAddress(walletInfo.solanaPublicKey, 12)}
                  </code>
                </div>
              )}
              {walletInfo.walletType === "loop" && walletInfo.publicKey && (
                <div className="flex items-center gap-2">
                  <span className="text-muted-foreground">Public Key:</span>
                  <code className="font-mono text-xs bg-muted px-2 py-0.5 rounded">
                    {truncateAddress(walletInfo.publicKey, 12)}
                  </code>
                </div>
              )}
              <div className="flex items-center gap-2">
                <span className="text-muted-foreground">Party ID:</span>
                <code className="font-mono text-xs bg-muted px-2 py-0.5 rounded">
                  {walletInfo.partyId ? truncateAddress(walletInfo.partyId, 16) : "N/A"}
                </code>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Main Content */}
      <main className="relative z-10 container mx-auto px-4 py-8">
        <LoopWalletDashboard
          loopPartyId={walletInfo.partyId}
          network={network}
          walletName={walletInfo.walletName || "Loop"}
          walletType={walletInfo.walletType}
        />
      </main>

      {/* Network Select Modal */}
      <NetworkSelectModal
        isOpen={showNetworkModal}
        onClose={() => setShowNetworkModal(false)}
        onConnect={handleNetworkSelect}
        selectedNetwork={network}
      />

      {/* Wallet Select Modal */}
      {showWalletModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center">
          {/* Backdrop */}
          <div
            className="absolute inset-0 bg-black/50 backdrop-blur-sm"
            onClick={() => setShowWalletModal(false)}
          />

          {/* Modal */}
          <div className="relative bg-background border border-border rounded-xl shadow-2xl w-full max-w-md mx-4 p-6">
            {/* Close button */}
            <button
              onClick={() => setShowWalletModal(false)}
              className="absolute top-4 right-4 p-1 rounded-md hover:bg-muted transition-colors"
            >
              <X className="h-5 w-5" />
            </button>

            {/* Header */}
            <div className="text-center mb-6">
              <h2 className="text-xl font-bold bg-gradient-to-r from-brand-pink via-brand-purple to-brand-blue text-transparent bg-clip-text">
                Connect Wallet
              </h2>
              <p className="text-sm text-muted-foreground mt-1">
                Choose your wallet to connect
              </p>
            </div>

            {/* Wallet Options */}
            <div className="space-y-3">
              {/* Loop Wallet */}
              <button
                onClick={handleConnectLoop}
                className="w-full flex items-center gap-4 p-4 rounded-lg border border-border hover:border-brand-purple hover:bg-muted/50 transition-all"
              >
                <div className="w-12 h-12 rounded-lg bg-muted flex items-center justify-center">
                  <Image
                    src="/loop.png"
                    alt="Loop"
                    width={32}
                    height={32}
                    className="rounded-sm"
                  />
                </div>
                <div className="text-left">
                  <div className="font-semibold">Loop</div>
                  <div className="text-sm text-muted-foreground">Canton Network</div>
                </div>
              </button>

              {/* Phantom Wallet */}
              <button
                onClick={handleConnectPhantom}
                className="w-full flex items-center gap-4 p-4 rounded-lg border border-border hover:border-brand-purple hover:bg-muted/50 transition-all"
              >
                <div className="w-12 h-12 rounded-lg bg-muted flex items-center justify-center">
                  <Image
                    src="/phantom.svg"
                    alt="Phantom"
                    width={32}
                    height={32}
                    className="rounded-sm"
                  />
                </div>
                <div className="text-left">
                  <div className="font-semibold">Phantom</div>
                  <div className="text-sm text-muted-foreground">Solana</div>
                </div>
              </button>

              {/* Solflare Wallet */}
              <button
                onClick={handleConnectSolflare}
                className="w-full flex items-center gap-4 p-4 rounded-lg border border-border hover:border-brand-purple hover:bg-muted/50 transition-all"
              >
                <div className="w-12 h-12 rounded-lg bg-muted flex items-center justify-center">
                  <Image
                    src="/solflare.svg"
                    alt="Solflare"
                    width={32}
                    height={32}
                    className="rounded-sm"
                  />
                </div>
                <div className="text-left">
                  <div className="font-semibold">Solflare</div>
                  <div className="text-sm text-muted-foreground">Solana</div>
                </div>
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
