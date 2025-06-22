"use client";

import dynamic from "next/dynamic";
import { useState, useEffect, useRef } from "react";
import { ModernHeader } from "@/components/dashboard/modern-header";
import { TeeStatusDashboard } from "@/components/dashboard/tee-status-dashboard";
import { UserStatusDashboard } from "@/components/dashboard/user-status-dashboard";
import { MinaWalletDashboard } from "@/components/dashboard/mina-wallet-dashboard";
import { WalletConnectModal } from "@/components/dashboard/wallet-connect-modal";
import { AnimatedBackground } from "@/components/ui/animated-background";
import { ModernCard, SectionHeader } from "@/components/ui/modern-card";
import type {
  UserWalletStatus,
  UserSocialLoginStatus,
  ApiFunctions,
} from "@/lib/types";
import {
  Attestation,
  TeeStats,
  TeeStatusData,
  getAttestation,
  getStats,
} from "@/lib/tee";
import { getWalletById } from "@/lib/wallet";
import Image from "next/image";
import type { ApiFrameHandle } from "@/components/api/api";
import { useUserState, UserStateProvider } from "@/context/userState";
import { sleep } from "@/lib/utils";

const Api = dynamic(() => import("@/components/api/api").then((m) => m.Api), {
  ssr: false,
});

// Internal dashboard component
function SilvanaTeeDashboardInternal(props: { apiFunctions: ApiFunctions }) {
  const { apiFunctions } = props;
  const [teeStatus, setTeeStatus] = useState<TeeStatusData | null>(null);
  const [isWalletModalOpen, setIsWalletModalOpen] = useState(false);
  const [isLoadingTee, setIsLoadingTee] = useState(true);
  const [isFetchingTee, setIsFetchingTee] = useState(false);

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
  } = useUserState();

  useEffect(() => {
    const fetchTeeData = async () => {
      if (isFetchingTee || !isLoadingTee) {
        return;
      }
      setIsFetchingTee(true);
      const stats = await getStats();
      const attestation = await getAttestation();
      if (
        !stats.success ||
        !attestation.success ||
        !stats.data ||
        !attestation.data
      ) {
        setIsLoadingTee(false);
        setIsFetchingTee(false);
        return;
      }
      await sleep(1000);
      const verifiedAttestation = await apiFunctions.verifyAttestation(
        attestation.data
      );
      if (
        !verifiedAttestation ||
        verifiedAttestation.error ||
        !verifiedAttestation.verifiedAttestation
      ) {
        setIsLoadingTee(false);
        setIsFetchingTee(false);
        return;
      }
      let attestationData: { result: Attestation; error: string | null };
      try {
        attestationData = JSON.parse(
          verifiedAttestation.verifiedAttestation
        ) as { result: Attestation; error: string | null };
      } catch (error) {
        console.error("Error parsing attestation data", error);
        setIsLoadingTee(false);
        setIsFetchingTee(false);
        return;
      }
      if (!attestationData.result) {
        setIsLoadingTee(false);
        setIsFetchingTee(false);
        return;
      }
      setTeeStatus({
        stats: stats.data,
        attestation: attestationData.result,
      });
      setIsLoadingTee(false);
      setIsFetchingTee(false);
    };
    fetchTeeData();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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
          teeConnected={teeStatus?.attestation.is_valid ?? false}
          teeLoading={isLoadingTee}
          onAddConnection={() => setIsWalletModalOpen(true)}
        />

        <div className="pt-20 pb-12">
          <div className="container mx-auto px-6 xl:px-12 max-w-[1440px]">
            <ModernCard delay={0.1} className="mb-6">
              <SectionHeader>Connected Wallets & Accounts</SectionHeader>

              {connectedMethods.length > 0 ? (
                <div className="flex flex-wrap items-center gap-4">
                  {connectedWallets.map(renderConnection)}
                  {connectedSocials.map(renderConnection)}
                </div>
              ) : (
                <p className="text-sm text-muted-foreground text-center">
                  Click &ldquo;Add Connection&rdquo; to connect your wallets and
                  social accounts
                </p>
              )}
            </ModernCard>

            {/* Main Grid Layout */}
            <div className="grid grid-cols-12 gap-6">
              {/* User Authentication - Left Column */}
              <div className="col-span-12 md:col-span-5 lg:col-span-4">
                <UserStatusDashboard />
              </div>

              {/* Mina Wallet - Right Column */}
              <div className="col-span-12 md:col-span-7 lg:col-span-8">
                <MinaWalletDashboard />
              </div>

              {/* TEE Status - Split into two columns on same row */}
              <div className="col-span-12 md:col-span-6">
                <TeeStatusDashboard
                  status={teeStatus}
                  isLoading={isLoadingTee}
                  sections={["system-resources", "tee-addresses"]}
                  title="TEE System & Addresses"
                />
              </div>

              <div className="col-span-12 md:col-span-6">
                <TeeStatusDashboard
                  status={teeStatus}
                  isLoading={isLoadingTee}
                  sections={["attestation-details", "pcr-values"]}
                  title="TEE Attestation & PCR"
                />
              </div>
            </div>
          </div>
        </div>

        {/* Wallet Connect Modal */}
        <WalletConnectModal
          isOpen={isWalletModalOpen}
          connect={connect}
          onClose={() => setIsWalletModalOpen(false)}
          getConnectionState={getConnectionState}
          setConnecting={setConnecting}
          setConnectionFailed={setConnectionFailed}
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
      console.log("signMessage button clicked", params);
      if (!apiRef.current || !publicKey || !message) {
        console.log("signMessage Api or publicKey or message not found");
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
      console.error("signMessage error:", error?.message);
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
      console.log("signPayment button clicked", params);
      if (!apiRef.current || !publicKey || !payment) {
        console.log("signPayment Api or publicKey or payment not found");
        return {
          signature: null,
          error: "Api or publicKey or payment not found",
        };
      }

      const signature = await apiRef.current.signPayment(payment, publicKey);
      console.log("signature:", signature);
      if (!signature) {
        return { signature: null, error: "Error E102 signing payment" };
      }
      return { signature, error: null };
    } catch (error: any) {
      console.error("signPayment error:", error?.message);
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
      console.error("verifyAttestation error:", error?.message);
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
        console.log("Api not found");
        return null;
      }
      console.log("decryptShares called with", data, privateKeyId);
      const publicKey = await apiRef.current.decryptShares(data, privateKeyId);
      console.log("publicKey:", publicKey);
      return publicKey;
    } catch (error: any) {
      console.error("decryptShares error:", error?.message);
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
