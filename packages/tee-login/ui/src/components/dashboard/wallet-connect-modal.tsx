"use client";

import { useEffect, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { X, Check } from "lucide-react";
import Image from "next/image";
import { WalletButtonProps, WalletOption, walletOptions } from "@/lib/wallet";
import { useSession, getSession } from "next-auth/react";
import { SocialLoginData, WalletType } from "@/lib/types";
import type { Session } from "next-auth";
import { useSocialLogin } from "@/hooks/use-social-login";
import { Logger } from "@logtail/next";

const log = new Logger({
  source: "WalletConnectModal",
});

function WalletButton({
  wallet,
  connected,
  loading,
  failed,
  onClick,
}: WalletButtonProps) {
  const getChainColor = (chain: string) => {
    switch (chain) {
      case "Ethereum":
        return "text-brand-blue";
      case "Solana":
        return "text-brand-purple";
      case "Sui":
        return "text-brand-pink";
      case "Social":
        return "text-brand-green";
      default:
        return "text-foreground";
    }
  };

  const getChainGradient = (chain: string) => {
    switch (chain) {
      case "ethereum":
        return "from-brand-blue/40 via-brand-blue/30 to-brand-blue/20";
      case "solana":
        return "from-brand-purple/40 via-brand-purple/30 to-brand-purple/20";
      case "sui":
        return "from-brand-pink/40 via-brand-pink/30 to-brand-pink/20";
      case "social":
        return "from-brand-green/40 via-brand-green/30 to-brand-green/20";
      default:
        return "from-brand-purple/30 via-brand-pink/20 to-brand-blue/30";
    }
  };

  return (
    <motion.button
      initial={{ scale: 0.9, opacity: 0 }}
      animate={{ scale: 1, opacity: 1 }}
      transition={{
        type: "spring",
        duration: 0.35,
        stiffness: 300,
        damping: 25,
      }}
      whileHover={{ y: -3, scale: 1.05 }}
      whileTap={{ scale: 0.95 }}
      onClick={onClick}
      disabled={loading || connected}
      className={`group relative aspect-square rounded-2xl min-w-[72px] min-h-[72px]
                 backdrop-blur-md transition-all duration-300 
                 focus:outline-none focus:ring-2 focus:ring-brand-pink focus:ring-offset-2 focus:ring-offset-transparent
                 disabled:cursor-not-allowed overflow-hidden border-2
                 ${
                   connected
                     ? "bg-gradient-to-br from-brand-green/60 via-brand-green/40 to-brand-green/20 border-brand-green text-white shadow-xl shadow-brand-green/40"
                     : failed
                     ? "bg-gradient-to-br from-red-500/60 via-red-500/40 to-red-500/20 border-red-500 text-white shadow-xl shadow-red-500/40"
                     : `bg-gradient-to-br ${getChainGradient(
                         wallet.type === "wallet" ? wallet.chain : "social"
                       )} border-white/50 text-foreground hover:border-brand-pink hover:shadow-xl hover:shadow-brand-purple/30`
                 }`}
      tabIndex={0}
    >
      {/* Bright animated gradient background */}
      <div className="absolute inset-0 bg-gradient-to-br from-brand-pink/20 via-brand-purple/20 to-brand-blue/20 opacity-0 group-hover:opacity-100 transition-opacity duration-300" />

      {/* Wallet Icon */}
      <div className="relative mb-2 z-10">
        <div className="w-8 h-8 mx-auto flex items-center justify-center bg-white/10 rounded-lg backdrop-blur-sm">
          <Image
            src={wallet.logo || "/placeholder.svg"}
            alt={`${wallet.name} logo`}
            width={24}
            height={24}
            className="w-6 h-6 object-contain drop-shadow-sm"
            onError={(e) => {
              // Fallback to placeholder if image fails to load
              e.currentTarget.src =
                "/placeholder.svg?height=24&width=24&text=" +
                wallet.name.substring(0, 2);
            }}
          />
        </div>

        {/* Loading spinner overlay - only show if loading and not failed */}
        {loading && !failed && (
          <div className="absolute inset-0 flex items-center justify-center bg-brand-purple/20 rounded backdrop-blur-sm">
            <div className="w-8 h-8 border-4 border-transparent border-t-brand-pink border-l-brand-purple rounded-full animate-spin" />
          </div>
        )}
      </div>

      {/* Wallet Name */}
      <div className="text-center px-2 relative z-10">
        <div className="text-[0.75rem] font-semibold leading-tight mb-1 text-foreground drop-shadow-md">
          {wallet.name}
        </div>
        <div className="text-[0.625rem] text-foreground/90 font-bold drop-shadow-md">
          {wallet.type === "wallet" ? wallet.chain : wallet.provider}
        </div>
      </div>

      {/* Connected Indicator */}
      {connected && (
        <div className="absolute top-1 right-1 w-6 h-6 bg-gradient-to-br from-brand-green via-brand-green to-brand-green/80 rounded-full flex items-center justify-center border-2 border-white shadow-xl shadow-brand-green/50">
          <Check className="w-3 h-3 text-white" strokeWidth={3} />
        </div>
      )}

      {/* Failed Indicator */}
      {failed && !connected && (
        <div className="absolute top-1 right-1 w-6 h-6 bg-gradient-to-br from-red-500 via-red-500 to-red-500/80 rounded-full flex items-center justify-center border-2 border-white shadow-xl shadow-red-500/50">
          <X className="w-3 h-3 text-white" strokeWidth={3} />
        </div>
      )}

      {/* Bright shine effect on hover */}
      <div className="absolute inset-0 rounded-2xl opacity-0 group-hover:opacity-100 transition-opacity  pointer-events-none bg-gradient-to-r from-transparent via-white/30 to-transparent transform -skew-x-12 translate-x-[-100%] group-hover:translate-x-[100%] duration-1000" />
    </motion.button>
  );
}

interface WalletConnectModalProps {
  isOpen: boolean;
  onClose: () => void;
  connect: (params: {
    walletId: string;
    socialLoginData?: SocialLoginData;
  }) => Promise<void>;
  getConnectionState: (walletId: string) => any;
  setConnecting: (walletId: string, walletType: WalletType) => void;
  setConnectionFailed: (walletId: string) => void;
  resetFailedConnections: () => void;
}

export function WalletConnectModal({
  isOpen,
  onClose,
  connect,
  getConnectionState,
  setConnecting,
  setConnectionFailed,
  resetFailedConnections,
}: WalletConnectModalProps) {
  const [successWallet, setSuccessWallet] = useState<string | null>(null);
  const openSocialLogin = useSocialLogin();
  const { data: session, update } = useSession();
  const [processSocialLogin, setProcessSocialLogin] =
    useState<WalletOption | null>(null); // trying to refresh the session due to next-auth bug https://github.com/nextauthjs/next-auth/issues/9504
  const [counter, setCounter] = useState<number>(0);

  useEffect(() => {
    async function processLogin() {
      if (processSocialLogin !== null) {
        const option = processSocialLogin;
        setProcessSocialLogin(null);
        await handleWalletClick(option, true);
      }
    }
    processLogin();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [processSocialLogin]);

  const closeModal = () => {
    resetFailedConnections();
    onClose();
  };

  const isConnected = (walletId: string) => {
    const state = getConnectionState(walletId);
    return state.state === "connected" || successWallet === walletId;
  };

  const isLoading = (walletId: string) => {
    const state = getConnectionState(walletId);
    return state.state === "connecting";
  };

  const isFailed = (walletId: string) => {
    const state = getConnectionState(walletId);
    return state.state === "error";
  };

  async function handleWalletClick(
    wallet: WalletOption,
    followUp: boolean = false
  ): Promise<boolean | undefined> {
    const currentState = getConnectionState(wallet.id);
    if (
      (!followUp && currentState.state === "connecting") ||
      currentState.state === "connected"
    )
      return undefined;

    try {
      console.log("handleWalletClick: connecting", wallet.id, wallet.name);
      const walletInfo = walletOptions.find((w) => w.id === wallet.id);
      if (!walletInfo) {
        log.error("Wallet not found T101", {
          walletId: wallet.id,
        });
        return;
      }
      if (walletInfo.type === "social") {
        console.log(
          "handleWalletClick: signing in",
          walletInfo.provider,
          session
        );
        const newSession = (await getSession()) as Session & {
          provider?: string;
          accessToken?: string;
          idToken?: string;
        };
        console.log("newSession", newSession);
        if (newSession?.provider !== walletInfo.provider) {
          if (counter < 5) {
            setCounter(counter + 1);
            await update();
            await new Promise((resolve) => setTimeout(resolve, 1000));
            setProcessSocialLogin(wallet); // trying to refresh the session due to next-auth bug 5 times
            return;
          }
          log.error("ERROR: wrong provider T102", {
            newSessionProvider: newSession?.provider,
            requestedProvider: walletInfo.provider,
            counter,
          });
          setConnectionFailed(wallet.id);
          return;
        }
        if (!newSession) {
          log.error("Session not found T103", {
            walletId: wallet.id,
          });
          setConnectionFailed(wallet.id);
          return;
        }
        const socialLoginData: SocialLoginData = {
          id: newSession.user?.id ?? walletInfo.provider,
          name: newSession.user?.name || undefined,
          email: newSession.user?.email || undefined,
          idToken: newSession.idToken,
          accessToken: newSession.accessToken,
          expires: newSession.expires,
        };
        console.log("handleWalletClick: socialLoginData", socialLoginData);
        await connect({ walletId: wallet.id, socialLoginData });
      } else {
        await connect({ walletId: wallet.id });
      }
      console.log("handleWalletClick: connected", wallet.id, wallet.name);
      const finalState = getConnectionState(wallet.id);
      console.log("handleWalletClick: finalState", finalState);

      if (finalState.state === "connected") {
        console.log("handleWalletClick: connected", wallet.id, wallet.name);
        return true;
      }
    } catch (error: any) {
      log.error("handleWalletClick: Connection failed T104", {
        error: error?.message,
        walletId: wallet.id,
      });
      setConnectionFailed(wallet.id);
      return false;
    }
  }

  return (
    <AnimatePresence>
      {isOpen && (
        <>
          {/* Vibrant Backdrop */}
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
            className="fixed inset-0 z-50 bg-gradient-to-br from-brand-pink/20 via-brand-purple/30 to-brand-blue/20 backdrop-blur-sm"
            onClick={closeModal}
          />

          {/* Modal */}
          <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-0">
            <motion.div
              initial={{ scale: 0.9, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              exit={{ scale: 0.9, opacity: 0 }}
              transition={{
                type: "spring",
                duration: 0.3,
                stiffness: 300,
                damping: 30,
              }}
              className="relative w-full max-w-[480px] sm:max-h-[80vh] sm:rounded-3xl
                         max-sm:fixed max-sm:bottom-0 max-sm:left-0 max-sm:right-0 max-sm:rounded-t-3xl max-sm:rounded-b-none
                         bg-gradient-to-br from-brand-pink/30 via-brand-purple/25 to-brand-blue/30 
                         backdrop-blur-xl border-2 border-white/40
                         shadow-[0_32px_96px_rgba(239,69,207,0.4)]
                         overflow-hidden"
            >
              {/* Bright animated gradient overlay */}
              <div className="absolute inset-0 bg-gradient-to-br from-brand-pink/10 via-brand-purple/20 to-brand-blue/10 animate-pulse" />

              {/* Header */}
              <div className="px-6 py-6 border-b border-white/30 relative z-10">
                <div className="flex items-center justify-between">
                  <div className="text-center flex-1">
                    <h2 className="text-2xl md:text-3xl font-bold text-foreground font-sans mb-2 drop-shadow-lg">
                      <span className="bg-gradient-to-r from-brand-pink via-brand-purple to-brand-blue bg-clip-text text-transparent">
                        Connect your Wallets &
                      </span>
                      <br />
                      <span className="bg-gradient-to-r from-brand-blue via-brand-purple to-brand-pink bg-clip-text text-transparent">
                        Accounts
                      </span>
                    </h2>
                    <p
                      className="text-sm text-white max-w-[340px] mx-auto font-medium drop-shadow-lg rounded-lg px-3 py-2 border border-white/40 shadow-xl"
                      style={{
                        background:
                          "linear-gradient(to right, rgba(239, 69, 207, 0.7), rgba(141, 117, 255, 0.7), rgba(95, 168, 255, 0.7))",
                      }}
                    >
                      Choose your preferred wallet or social account to get
                      started
                    </p>
                  </div>
                  <button
                    onClick={closeModal}
                    className="absolute top-4 right-4 p-2 text-pink-500/80 hover:text-foreground transition-all duration-200 rounded-xl hover:bg-white/20 hover:shadow-lg hover:scale-110"
                  >
                    <X className="w-5 h-5" />
                  </button>
                </div>
              </div>

              {/* Wallet Grid */}
              <div
                className="grid grid-cols-2 min-[340px]:grid-cols-3 sm:grid-cols-4 gap-4 p-6 overflow-y-auto max-h-[70vh] relative z-10"
                role="grid"
                aria-label="Wallet selection grid"
              >
                {walletOptions.map((wallet) => (
                  <WalletButton
                    key={wallet.id}
                    wallet={wallet}
                    connected={isConnected(wallet.id)}
                    loading={isLoading(wallet.id)}
                    failed={isFailed(wallet.id)}
                    onClick={async () => {
                      setConnecting(wallet.id, wallet.type);
                      if (wallet.type === "social") {
                        setCounter(0);
                        await openSocialLogin(wallet.provider);
                        await new Promise((resolve) =>
                          setTimeout(resolve, 100)
                        );
                        await update(); // does not work in many cases to to next-auth bug
                        setProcessSocialLogin(wallet);
                        return;
                      } else {
                        setProcessSocialLogin(null);
                        handleWalletClick(wallet);
                      }
                    }}
                  />
                ))}
              </div>
            </motion.div>
          </div>
        </>
      )}
    </AnimatePresence>
  );
}
