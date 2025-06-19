"use client";

import dynamic from "next/dynamic";
import { useState, useEffect, useRef } from "react";
import { ModernHeader } from "@/components/dashboard/modern-header";
import { TeeStatusDashboard } from "@/components/dashboard/tee-status-dashboard";
import { UserStatusDashboard } from "@/components/dashboard/user-status-dashboard";
import { MinaWalletDashboard } from "@/components/dashboard/mina-wallet-dashboard";
import { WalletConnectModal } from "@/components/ui/wallet-connect-modal";
import { AnimatedBackground } from "@/components/ui/animated-background";
import { ModernCard, SectionHeader } from "@/components/ui/modern-card";
import type {
  TeeStatusData,
  UserWalletStatus,
  UserSocialLoginStatus,
  ApiFunctions,
} from "@/lib/types";
import { getWalletById } from "@/lib/wallet";
import { Github, Globe, Activity } from "lucide-react";
import Image from "next/image";
import { AuthComponent, SocialLoginFunction } from "@/components/auth/auth";
import type { ApiFrameHandle } from "@/components/api/api";
import { useSession } from "next-auth/react";
import { useUserState, UserStateProvider } from "@/context/userState";
import { useTheme } from "next-themes";

const Api = dynamic(() => import("@/components/api/api").then((m) => m.Api), {
  ssr: false,
});

// Mock data and functions
const mockTeeStatsResponse = {
  response: {
    intent: 1,
    timestamp_ms: 1750233231496,
    data: {
      cpu_cores: 8,
      memory: 8140572,
      available_memory: 7886012,
      free_memory: 8020332,
      used_memory: 254560,
      timestamp: "2025-06-18T07:53:51.496+00:00",
    },
  },
  signature:
    "cfbb777c6712e54e58e16745c4f96288f09c28f6ea90f399cb767835da1e79207c4eeb7d22470869f86ee38c48e7443569c8178035a6aaa5956574b54f92f30d",
};

// Internal dashboard component
function SilvanaTeeDashboardInternal() {
  const [teeStatus, setTeeStatus] = useState<TeeStatusData | null>(null);
  const [isWalletModalOpen, setIsWalletModalOpen] = useState(false);
  const [isLoadingTee, setIsLoadingTee] = useState(true);
  const [signedMessage, setSignedMessage] = useState<{
    msg: bigint[];
    signature: string;
  } | null>(null);
  const { data: session } = useSession();
  const { resolvedTheme } = useTheme();

  // Get state and methods from context
  const {
    state: userState,
    connect,
    getConnectionState,
    setSelectedAuthMethod,
    getWalletConnections,
    getSocialConnections,
    getConnectedMethods,
  } = useUserState();

  useEffect(() => {
    console.log("SilvanaTeeDashboard userState", userState);
  }, [userState]);

  useEffect(() => {
    const fetchTeeData = async () => {
      setIsLoadingTee(true);
      await new Promise((resolve) => setTimeout(resolve, 1500));
      setTeeStatus({
        stats: mockTeeStatsResponse.response.data,
        attestation: {
          is_valid: true,
          digest: "SHA384",
          timestamp: 1750233174339,
          module_id: "i-004c4acc95caeb12a-enc01978206707527d6",
          pcr_vec: [
            "b9d08361baa85f592c98b491f1982caaf03f5b1fb8a2a76452f5754510c6864dc88cfa146d43704c9ff9911a2b822883",
            "b9d08361baa85f592c98b491f1982caaf03f5b1fb8a2a76452f5754510c6864dc88cfa146d43704c9ff9911a2b822883",
            "21b9efbc184807662e966d34f390821309eeac6802309798826296bf3e8bec7c10edb30948c90ba67310f7b964fc500a",
          ],
          pcr_map: {
            0: "b9d08361baa85f592c98b491f1982caaf03f5b1fb8a2a76452f5754510c6864dc88cfa146d43704c9ff9911a2b822883",
            1: "b9d08361baa85f592c98b491f1982caaf03f5b1fb8a2a76452f5754510c6864dc88cfa146d43704c9ff9911a2b822883",
            2: "21b9efbc184807662e966d34f390821309eeac6802309798826296bf3e8bec7c10edb30948c90ba67310f7b964fc500a",
          },
          pcr_locked: {
            0: true,
            1: true,
            2: false,
            3: true,
            4: true,
            8: false,
          },
          addresses: {
            solana_address: "AUJTAeQFrVEoRjKjsKRHaW1aiJG2A5BceSTvGZfpcP1S",
            sui_address:
              "0xa9785af780b16b646041d260c19b2087cac4ffeff636b0347f0b07eee8b0d8f1",
            mina_address:
              "B62qqngPFeyNniTX8yaTA8S5MxuM2FZrFb2VEsZ3oZ3HudKLBCs4Em3",
            ethereum_address: "0x0ea8643911f36cc73b473735ca2578bb070598b0",
          },
        },
      });
      setIsLoadingTee(false);
    };
    fetchTeeData();
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
          onAddConnection={() => setIsWalletModalOpen(true)}
        />

        <div className="pt-20 pb-12">
          <div className="container mx-auto px-6 xl:px-12 max-w-[1440px]">
            {/* Connected Wallets Section */}
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
          onClose={() => setIsWalletModalOpen(false)}
          connect={connect}
          getConnectionState={getConnectionState}
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
      const signature = await apiRef.current.sign(msg, publicKey);

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
  };

  return (
    <>
      <UserStateProvider apiFunctions={apiFunctions}>
        <SilvanaTeeDashboardInternal />
      </UserStateProvider>
      <Api ref={apiRef} />
    </>
  );
}
