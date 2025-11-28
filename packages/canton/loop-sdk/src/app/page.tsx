"use client";

import { useState } from "react";
import { LoopWalletDashboard } from "@/components/dashboard/loop-wallet-dashboard";
import { NetworkSelectModal, type NetworkType } from "@/components/dashboard/network-select-modal";
import { Button } from "@/components/ui/button";
import { AnimatedBackground } from "@/components/ui/animated-background";
import { useUserState } from "@/context/userState";
import { Globe, Wallet, LogOut } from "lucide-react";
import { disconnectLoop } from "@/lib/loop";

export default function HomePage() {
  const [network, setNetwork] = useState<NetworkType>("devnet");
  const [showNetworkModal, setShowNetworkModal] = useState(false);
  const { connectLoop, getLoopPartyId } = useUserState();

  const loopPartyId = getLoopPartyId();
  const isConnected = !!loopPartyId;

  const handleNetworkSelect = (selectedNetwork: NetworkType) => {
    setNetwork(selectedNetwork);
    setShowNetworkModal(false);
  };

  const handleConnect = async () => {
    await connectLoop(network);
  };

  const handleLogout = () => {
    disconnectLoop();
    window.location.reload();
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
                <Button
                  onClick={handleLogout}
                  className="bg-gradient-to-r from-brand-pink via-brand-purple to-brand-blue hover:brightness-110 text-white"
                >
                  <LogOut className="h-4 w-4 mr-2" />
                  Logout
                </Button>
              ) : (
                <Button
                  onClick={handleConnect}
                  className="bg-gradient-to-r from-brand-pink via-brand-purple to-brand-blue hover:brightness-110 text-white"
                >
                  <Wallet className="h-4 w-4 mr-2" />
                  Connect Loop
                </Button>
              )}
            </div>
          </div>
        </div>
      </header>

      {/* Main Content */}
      <main className="relative z-10 container mx-auto px-4 py-8">
        <LoopWalletDashboard loopPartyId={loopPartyId} network={network} />
      </main>

      {/* Network Select Modal */}
      <NetworkSelectModal
        isOpen={showNetworkModal}
        onClose={() => setShowNetworkModal(false)}
        onConnect={handleNetworkSelect}
        selectedNetwork={network}
      />
    </div>
  );
}
