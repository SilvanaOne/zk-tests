"use client";

import dynamic from "next/dynamic";
import { useState, useEffect, useRef } from "react";
import { ModernHeader } from "@/components/dashboard/modern-header";
import { UserStatusDashboard } from "@/components/dashboard/user-status-dashboard";
import { LoopWalletDashboard } from "@/components/dashboard/loop-wallet-dashboard";
import { WalletConnectModal } from "@/components/dashboard/wallet-connect-modal";
import { AnimatedBackground } from "@/components/ui/animated-background";
import { ModernCard, SectionHeader } from "@/components/ui/modern-card";
import type {
  UserWalletStatus,
  UserSocialLoginStatus,
  ApiFunctions,
} from "@/lib/types";
import { getWalletById } from "@/lib/wallet";
import Image from "next/image";
import type { ApiFrameHandle } from "@/components/api/api";
import { useUserState, UserStateProvider } from "@/context/userState";
import { sleep } from "@/lib/utils";
import { Logger } from "@logtail/next";
import { initLoop, connectLoop, disconnectLoop, type LoopNetwork } from "@/lib/loop";
import {
  NetworkSelectModal,
  type NetworkType,
} from "@/components/dashboard/network-select-modal";

const log = new Logger({
  source: "SilvanaTeeDashboard",
});

const Api = dynamic(() => import("@/components/api/api").then((m) => m.Api), {
  ssr: false,
});

// Internal dashboard component
function SilvanaTeeDashboardInternal(props: { apiFunctions: ApiFunctions }) {
  const { apiFunctions } = props;
  const [isWalletModalOpen, setIsWalletModalOpen] = useState(false);

  // Loop wallet state
  const [loopConnected, setLoopConnected] = useState(false);
  const [loopPartyId, setLoopPartyId] = useState<string | null>(null);
  const [loopInitialized, setLoopInitialized] = useState(false);
  const [isNetworkModalOpen, setIsNetworkModalOpen] = useState(false);
  const [selectedNetwork, setSelectedNetwork] = useState<NetworkType>("devnet");

  // Get state and methods from context
  const {
    state: userState,
    connect,
    getConnectionState,
    setSelectedAuthMethod,
    setConnecting,
    setConnectionFailed,
    getWalletConnections,
    getSocialConnections,
    getConnectedMethods,
    resetFailedConnections,
  } = useUserState();

  // Initialize Loop SDK
  useEffect(() => {
    if (loopInitialized) return;

    initLoop({
      onConnect: (provider) => {
        console.log("Loop wallet connected:", provider.party_id);
        setLoopConnected(true);
        setLoopPartyId(provider.party_id);
      },
      onReject: () => {
        console.log("Loop wallet connection rejected");
        setLoopConnected(false);
        setLoopPartyId(null);
      },
    });
    setLoopInitialized(true);
  }, [loopInitialized]);

  // Handle Connect button click - opens network selection modal
  const handleConnect = () => {
    setIsNetworkModalOpen(true);
  };

  // Handle network selection and connect
  const handleNetworkConnect = (network: NetworkType) => {
    setSelectedNetwork(network);
    setIsNetworkModalOpen(false);
    connectLoop(network as LoopNetwork);
  };

  // Handle logout
  const handleLogout = () => {
    disconnectLoop();
    setLoopConnected(false);
    setLoopPartyId(null);
  };

  // Get connected wallets and social connections
  const connectedWallets = getWalletConnections();
  const connectedSocials = getSocialConnections();
  const connectedMethods = getConnectedMethods();

  // Helper function to render connection with proper wallet info
  const renderConnection = (
    connection: UserWalletStatus | UserSocialLoginStatus
  ) => {
    const walletInfo = getWalletById(connection.walletId || "");
    if (!walletInfo) return null;
    const chain =
      (connection as UserWalletStatus)?.chain?.[0]?.toUpperCase() +
      (connection as UserWalletStatus)?.chain?.slice(1);

    return (
      <div
        key={connection.walletId}
        className="flex items-center gap-2 bg-white/5 rounded-lg px-3 py-2"
      >
        <Image
          src={walletInfo.logo}
          alt={walletInfo.name}
          width={20}
          height={20}
          className="rounded"
          onError={(e) => {
            // Fallback to placeholder if logo fails to load
            e.currentTarget.src = `/placeholder.svg?height=20&width=20&text=${walletInfo.name.substring(
              0,
              2
            )}`;
          }}
        />
        <span className="text-sm text-foreground">
          {connection.loginType === "wallet"
            ? `${walletInfo.name} (${chain ?? ""})`
            : walletInfo.name}
        </span>
      </div>
    );
  };

  return (
    <>
      <AnimatedBackground />

      <div className="min-h-screen">
        <ModernHeader
          teeConnected={loopConnected}
          teeLoading={false}
          onAddConnection={handleConnect}
          onLogout={handleLogout}
        />

        <div className="pt-20 pb-12">
          <div className="container mx-auto px-6 xl:px-12 max-w-[1440px]">
            {/* Connected Wallets & Accounts - Hidden */}

            {/* Main Grid Layout */}
            <div className="grid grid-cols-12 gap-6">
              {/* User Authentication - Hidden for now */}

              {/* Loop Wallet - Full Width */}
              <div className="col-span-12">
                <LoopWalletDashboard loopPartyId={loopPartyId} network={selectedNetwork} />
              </div>

              {/* TEE Status - Hidden for now */}
            </div>
          </div>
        </div>

        {/* Network Select Modal */}
        <NetworkSelectModal
          isOpen={isNetworkModalOpen}
          onClose={() => setIsNetworkModalOpen(false)}
          onConnect={handleNetworkConnect}
          selectedNetwork={selectedNetwork}
        />

        {/* Wallet Connect Modal */}
        <WalletConnectModal
          isOpen={isWalletModalOpen}
          connect={connect}
          onClose={() => setIsWalletModalOpen(false)}
          getConnectionState={getConnectionState}
          setConnecting={setConnecting}
          setConnectionFailed={setConnectionFailed}
          resetFailedConnections={resetFailedConnections}
        />
      </div>
    </>
  );
}

// Main wrapper component with provider
export default function SilvanaTeeDashboard() {
  const apiRef = useRef<ApiFrameHandle>(null);
  async function signMessage(params: {
    publicKey: string;
    message: string;
  }): Promise<{ signature: string | null; error: string | null }> {
    try {
      const { publicKey, message } = params;
      log.info("signMessage button clicked T107", {
        publicKey,
        message,
      });
      if (!apiRef.current || !publicKey || !message) {
        log.error("signMessage Api or publicKey or message not found T106", {
          publicKey,
          message,
        });
        return {
          signature: null,
          error: "Api or publicKey or message not found",
        };
      }

      const msg: bigint[] = Array.from(new TextEncoder().encode(message)).map(
        (b) => BigInt(b)
      );
      const signature = await apiRef.current.signMessage(msg, publicKey);

      console.log("signature:", signature);
      return { signature, error: null };
    } catch (error: any) {
      log.error("signMessage error T108", {
        publicKey: params.publicKey,
        error: error?.message,
      });
      return {
        signature: null,
        error: error?.message || "Error E101 signing message",
      };
    }
  }

  async function signPayment(params: {
    publicKey: string;
    payment: string;
  }): Promise<{ signature: string | null; error: string | null }> {
    try {
      const { publicKey, payment } = params;
      log.info("signPayment button clicked T109", {
        publicKey,
        payment,
      });
      if (!apiRef.current || !publicKey || !payment) {
        log.error("signPayment Api or publicKey or payment not found T110", {
          publicKey,
          payment,
        });
        return {
          signature: null,
          error: "Api or publicKey or payment not found",
        };
      }

      const signature = await apiRef.current.signPayment(payment, publicKey);
      if (!signature) {
        log.error("signPayment error T111", {
          publicKey,
          payment,
        });
        return { signature: null, error: "Error E102 signing payment" };
      }
      return { signature, error: null };
    } catch (error: any) {
      log.error("signPayment error T112", {
        publicKey: params.publicKey,
        error: error?.message,
      });
      return {
        signature: null,
        error: error?.message || "Error E103 signing payment",
      };
    }
  }

  async function getPrivateKeyId(): Promise<{
    privateKeyId: string;
    publicKey: string;
  } | null> {
    try {
      console.log("getPrivateKeyId called");
      if (!apiRef.current) {
        console.log("Api not found");
        return null;
      }

      const { privateKeyId, publicKey } = await apiRef.current.privateKeyId();

      console.log("privateKeyId:", privateKeyId);
      console.log("publicKey:", publicKey);
      return { privateKeyId, publicKey };
    } catch (error: any) {
      console.error("getPrivateKeyId error:", error?.message);
      return null;
    }
  }

  async function verifyAttestation(attestation: string): Promise<{
    verifiedAttestation: string | null;
    error: string | null;
  } | null> {
    try {
      if (!apiRef.current) {
        console.log("Api not found");
        return { verifiedAttestation: null, error: "Api not found" };
      }
      // await sleep(100);
      // const { privateKeyId } = await apiRef.current.privateKeyId();
      // console.log("privateKeyId:", privateKeyId);
      await sleep(100);
      console.log("Calling verifyAttestation");

      const verifiedAttestation = await apiRef.current.verifyAttestation(
        attestation
      );

      return { verifiedAttestation, error: null };
    } catch (error: any) {
      log.error("verifyAttestation error T113", {
        error: error?.message,
      });
      return { verifiedAttestation: null, error: error?.message };
    }
  }

  async function decryptShares(
    data: string[],
    privateKeyId: string
  ): Promise<string | null> {
    try {
      console.log("decryptShares called");
      if (!apiRef.current) {
        log.error("decryptShares Api not found T114", {
          data,
          privateKeyId,
        });
        return null;
      }
      console.log("decryptShares called with", data, privateKeyId);
      const publicKey = await apiRef.current.decryptShares(data, privateKeyId);
      console.log("publicKey:", publicKey);
      return publicKey;
    } catch (error: any) {
      log.error("decryptShares error T115", {
        error: error?.message,
      });
      return null;
    }
  }

  const apiFunctions: ApiFunctions = {
    getPrivateKeyId,
    decryptShares,
    signMessage,
    signPayment,
    verifyAttestation,
  };

  return (
    <>
      <UserStateProvider apiFunctions={apiFunctions}>
        <SilvanaTeeDashboardInternal apiFunctions={apiFunctions} />
      </UserStateProvider>
      <Api ref={apiRef} />
    </>
  );
}
