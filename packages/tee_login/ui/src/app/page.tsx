"use client";
import dynamic from "next/dynamic";
import { getWallets } from "@mysten/wallet-standard";
import { useEffect, useRef, useState } from "react";
import { connectPhantom, signPhantomMessage } from "@/lib/phantom";
import { connectSolflare, signSolflareMessage } from "@/lib/solflare";
import { connectMetaMask, signMetaMaskMessage } from "@/lib/metamask";
import {
  login,
  getMessage,
  LoginRequest,
  UnsignedLoginRequest,
  LoginResponse,
} from "@/lib/login";
import { AuthComponent, SocialLoginFunction } from "@/components/auth/auth";
import { useSession } from "next-auth/react";
import type { ApiFrameHandle } from "@/components/api/api";

const Api = dynamic(() => import("@/components/api/api").then((m) => m.Api), {
  ssr: false,
});

export default function Home() {
  const [chain, setChain] = useState<string | undefined>(undefined);
  const [wallet, setWallet] = useState<string | undefined>(undefined);
  const [message, setMessage] = useState<string | undefined>(undefined);
  const [signature, setSignature] = useState<string | undefined>(undefined);
  const [address, setAddress] = useState<string | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);
  const [loginSuccess, setLoginSuccess] = useState<boolean | null>(null);
  const [loginProcessing, setLoginProcessing] = useState<boolean>(false);
  const [shares, setShares] = useState<number[] | null | undefined>(null);
  const [publicKey, setPublicKey] = useState<string | null>(null);
  const [loginData, setLoginData] = useState<{
    request: LoginRequest;
    privateKeyId: string;
  } | null>(null);
  const [user, setUser] = useState<any | null>(null);
  const [signedMessage, setSignedMessage] = useState<{
    msg: bigint[];
    signature: string;
  } | null>(null);
  const apiRef = useRef<ApiFrameHandle>(null);

  const { data: session } = useSession();

  const signMessage = async () => {
    console.log("signMessage button clicked");
    setSignedMessage(null);
    if (!apiRef.current || !publicKey) {
      console.log("signMessage Api or publicKey not found");
      return;
    }

    const msg = [1n, 2n, 3n];
    const signature = await apiRef.current.sign(msg, publicKey);

    console.log("signature:", signature);
    setSignedMessage({ msg, signature });
  };

  async function getPrivateKeyId(): Promise<{
    privateKeyId: string;
    publicKey: string;
  } | null> {
    console.log("getPrivateKeyId button clicked");
    if (!apiRef.current) {
      console.log("Api not found");
      return null;
    }

    const { privateKeyId, publicKey } = await apiRef.current.privateKeyId();

    console.log("privateKeyId:", privateKeyId);
    console.log("publicKey:", publicKey);
    return { privateKeyId, publicKey };
  }

  async function decryptShares(
    data: string[],
    privateKeyId: string
  ): Promise<string | null> {
    console.log("decryptShares");
    if (!apiRef.current) {
      console.log("Api not found");
      return null;
    }
    console.log("decryptShares 2", data, privateKeyId);
    const publicKey = await apiRef.current.decryptShares(data, privateKeyId);
    console.log("publicKey:", publicKey);
    return publicKey;
  }

  useEffect(() => {
    if (session) {
      console.log("Session", session);
      setUser(session);
    }
  }, [session]);

  useEffect(() => {
    if (loginData) {
      setLoginProcessing(true);
      (async () => {
        const result = await login(loginData.request);
        console.log("Login result", result);
        setError(result.error);
        setLoginSuccess(result.success);
        setShares(result.indexes);
        if (result.data) {
          const publicKey = await decryptShares(
            result.data,
            loginData.privateKeyId
          );
          if (publicKey) {
            console.log("publicKey:", publicKey);
            setPublicKey(publicKey);
          }
        }
        setLoginData(null);
        setLoginProcessing(false);
      })();
    }
  }, [loginData]);

  function clear() {
    setChain(undefined);
    setWallet(undefined);
    setMessage(undefined);
    setSignature(undefined);
    setAddress(undefined);
    setError(null);
    setLoginSuccess(null);
    setShares(null);
    setLoginProcessing(false);
    setLoginData(null);
    setPublicKey(null);
    setSignedMessage(null);
  }

  const socialLogin: SocialLoginFunction = async (params) => {
    const { address, result, provider } = params;
    clear();
    setChain("Social login");
    setWallet(provider === "google" ? "Google" : "GitHub");
    setAddress(address);
    console.log(`${provider} login result:`, result);
    setError(result.error);
    setLoginSuccess(result.success);
    setShares(result.indexes);
    setPublicKey(result.publicKey ?? null);
    setLoginData(null);
    setLoginProcessing(false);
    setSignedMessage(null);
  };

  const handlePhantomSuiClick = async () => {
    clear();
    console.log("Phantom Sui button clicked");
    setChain("sui");
    setWallet("Phantom");

    const address = await connectPhantom("sui");
    console.log("Sui Address", address);
    setAddress(address);
    if (!address) {
      setError("No address");
      return;
    }

    const pk_result = await getPrivateKeyId();
    if (!pk_result) {
      setError("No private key id");
      return;
    }
    const msgData = await getMessage({
      login_type: "wallet",
      chain: "sui",
      wallet: "Phantom",
      address,
      publicKey: pk_result.publicKey,
    });
    if (!msgData) {
      setError("No request");
      return;
    }
    setMessage(msgData.request.message);

    const signedMessage = await signPhantomMessage({
      chain: "sui",
      message: msgData.request.message,
      display: "utf8",
    });
    if (!signedMessage) {
      setError("User rejected message");
      return;
    }
    console.log("Sui Signed message", signedMessage);
    const publicKey = (signedMessage as any)?.publicKey?.toString();
    console.log("Sui Public key", publicKey);
    const signature = (signedMessage as any)?.signature?.toString("hex");
    console.log("Sui Signature", signature);
    setSignature(signature);
    const request: LoginRequest = {
      ...msgData.request,
      signature,
    };
    setLoginData({
      request,
      privateKeyId: pk_result.privateKeyId,
    });
  };

  const handlePhantomSolanaClick = async () => {
    clear();
    console.log("Phantom Solana button clicked");
    setChain("solana");
    setWallet("Phantom");

    const address = await connectPhantom("solana");
    console.log("Solana Address", address);
    setAddress(address);
    if (!address) {
      setError("No address");
      return;
    }

    const pk_result = await getPrivateKeyId();
    if (!pk_result) {
      setError("No private key id");
      return;
    }
    const msgData = await getMessage({
      login_type: "wallet",
      chain: "solana",
      wallet: "Phantom",
      address,
      publicKey: pk_result.publicKey,
    });
    if (!msgData) {
      setError("No request");
      return;
    }
    setMessage(msgData.request.message);

    const signedMessage = await signPhantomMessage({
      chain: "solana",
      message: msgData.request.message,
      display: "utf8",
    });
    if (!signedMessage) {
      setError("User rejected message");
      return;
    }
    console.log("Solana Signed message", signedMessage);
    const publicKey = (signedMessage as any)?.publicKey?.toString();
    console.log("Solana Public key", publicKey);
    const signature = (signedMessage as any)?.signature?.toString("hex");
    console.log("Solana Signature", signature);
    setSignature(signature);
    const request: LoginRequest = {
      ...msgData.request,
      signature,
    };
    setLoginData({
      request,
      privateKeyId: pk_result.privateKeyId,
    });
  };

  const handlePhantomEthereumClick = async () => {
    clear();
    console.log("Phantom Ethereum button clicked");
    setChain("ethereum");
    setWallet("Phantom");

    const address = await connectPhantom("ethereum");
    console.log("Ethereum Address", address);
    setAddress(address);
    if (!address) {
      setError("No address");
      return;
    }

    const pk_result = await getPrivateKeyId();
    if (!pk_result) {
      setError("No private key id");
      return;
    }
    const msgData = await getMessage({
      login_type: "wallet",
      chain: "ethereum",
      wallet: "Phantom",
      address,
      publicKey: pk_result.publicKey,
    });
    if (!msgData) {
      setError("No request");
      return;
    }
    setMessage(msgData.request.message);

    const signedMessage = await signPhantomMessage({
      chain: "ethereum",
      message: msgData.request.message,
      display: "utf8",
    });
    if (!signedMessage) {
      setError("User rejected message");
      return;
    }
    console.log("Ethereum Signed message", signedMessage);
    const publicKey = (signedMessage as any)?.publicKey?.toString();
    console.log("Ethereum address", address);
    const signature = signedMessage;
    console.log("Ethereum Signature", signature);
    setSignature(signature);
    const request: LoginRequest = {
      ...msgData.request,
      signature,
    };
    setLoginData({
      request,
      privateKeyId: pk_result.privateKeyId,
    });
  };

  const handleMetaMaskClick = async () => {
    clear();
    console.log("MetaMask button clicked");
    setChain("ethereum");
    setWallet("MetaMask");

    const address = await connectMetaMask();
    console.log("Ethereum Address", address);
    setAddress(address);
    if (!address) {
      setError("No address");
      return;
    }

    const pk_result = await getPrivateKeyId();
    if (!pk_result) {
      setError("No private key id");
      return;
    }
    const msgData = await getMessage({
      login_type: "wallet",
      chain: "ethereum",
      wallet: "MetaMask",
      address,
      publicKey: pk_result.publicKey,
    });
    if (!msgData) {
      setError("No request");
      return;
    }
    setMessage(msgData.request.message);

    const signedMessage = await signMetaMaskMessage({
      message: msgData.request.message,
      display: "utf8",
    });
    if (!signedMessage) {
      setError("User rejected message");
      return;
    }
    console.log("Ethereum MetaMask Signed message", signedMessage);
    const publicKey = address;
    console.log("Ethereum MetaMask address", publicKey);
    const signature = signedMessage;
    console.log("Ethereum MetaMask Signature", signature);
    setSignature(signature);
    const request: LoginRequest = {
      ...msgData.request,
      signature,
    };
    setLoginData({
      request,
      privateKeyId: pk_result.privateKeyId,
    });
  };

  const handleSlushClick = async () => {
    clear();
    console.log("Slush button clicked");
    setChain("sui");
    setWallet("Slush");

    const availableWallets = getWallets().get();
    console.log("Available wallets", availableWallets);
    const wallet = availableWallets.find(
      (wallet) => wallet.name === "Slush"
    ) as any;
    console.log("Slush wallet", wallet);
    const connected = await wallet?.features["standard:connect"].connect(); // connect call
    console.log("Connected", connected);
    const address = connected?.accounts[0]?.address;
    console.log("Slush Address", address);
    setAddress(address);
    if (!address) {
      setError("No address");
      return;
    }

    const pk_result = await getPrivateKeyId();
    if (!pk_result) {
      setError("No private key id");
      return;
    }
    const msgData = await getMessage({
      login_type: "wallet",
      chain: "sui",
      wallet: "Slush",
      address,
      publicKey: pk_result.publicKey,
    });
    if (!msgData) {
      setError("No request");
      return;
    }
    setMessage(msgData.request.message);
    const message = new TextEncoder().encode(msgData.request.message);
    console.log("Message bytes", message);
    const signedMessage = await wallet?.features[
      "sui:signPersonalMessage"
    ].signPersonalMessage({
      message,
      account: connected.accounts[0],
      chain: "sui:mainnet",
    });
    if (!signedMessage) {
      setError("User rejected message");
      return;
    }
    console.log("Slush Signed message", signedMessage);
    setSignature(signedMessage?.signature);
    const request: LoginRequest = {
      ...msgData.request,
      signature: signedMessage?.signature,
    };
    setLoginData({
      request,
      privateKeyId: pk_result.privateKeyId,
    });
  };

  const handleSolflareClick = async () => {
    clear();
    console.log("Solflare button clicked");
    setChain("solana");
    setWallet("Solflare");

    const address = await connectSolflare();
    console.log("Solflare Address", address);
    setAddress(address);
    if (!address) {
      setError("No address");
      return;
    }

    const pk_result = await getPrivateKeyId();
    if (!pk_result) {
      setError("No private key id");
      return;
    }
    const msgData = await getMessage({
      login_type: "wallet",
      chain: "solana",
      wallet: "Solflare",
      address,
      publicKey: pk_result.publicKey,
    });
    if (!msgData) {
      setError("No request");
      return;
    }
    setMessage(msgData.request.message);
    const signedMessage = await signSolflareMessage({
      message: msgData.request.message,
    });
    if (!signedMessage) {
      setError("User rejected message");
      return;
    }
    console.log("Solflare Signed message", signedMessage);
    const publicKey = signedMessage?.publicKey;
    console.log("Solflare Public key", publicKey);
    const signature = signedMessage?.signature;
    console.log("Solflare Signature", signature);
    setSignature(signature);
    const request: LoginRequest = {
      ...msgData.request,
      signature,
    };
    setLoginData({
      request,
      privateKeyId: pk_result.privateKeyId,
    });
  };

  return (
    <div className="min-h-screen p-8 pb-20 sm:p-20 font-[family-name:var(--font-geist-sans)]">
      <main className="flex flex-col gap-4 items-center sm:items-start">
        {/* Connection Status Display */}
        <div className="w-full max-w-4xl bg-gray-50 dark:bg-gray-900 p-6 rounded-lg border">
          <h2 className="text-xl font-bold mb-4">Connection Status</h2>
          <Api ref={apiRef} />
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4 text-sm">
            <div>
              <span className="font-semibold">Chain:</span>{" "}
              <span className="font-mono bg-gray-200 dark:bg-gray-800 px-2 py-1 rounded">
                {chain || "Not connected"}
              </span>
            </div>
            <div>
              <span className="font-semibold">Wallet:</span>{" "}
              <span className="font-mono bg-gray-200 dark:bg-gray-800 px-2 py-1 rounded">
                {wallet || "Not connected"}
              </span>
            </div>
            <div className="md:col-span-2">
              <span className="font-semibold">Address:</span>{" "}
              <span className="font-mono bg-gray-200 dark:bg-gray-800 px-2 py-1 rounded break-all">
                {address || "Not connected"}
              </span>
            </div>
            <div className="md:col-span-2">
              <span className="font-semibold">Message:</span>{" "}
              <span className="font-mono bg-gray-200 dark:bg-gray-800 px-2 py-1 rounded break-all">
                {message || "No message"}
              </span>
            </div>
            <div className="md:col-span-2">
              <span className="font-semibold">Signature:</span>{" "}
              <span className="font-mono bg-gray-200 dark:bg-gray-800 px-2 py-1 rounded break-all text-xs">
                {signature || "No signature"}
              </span>
            </div>
            <div className="md:col-span-2">
              <span className="font-semibold">Login Status:</span>{" "}
              <span
                className={`font-mono px-2 py-1 rounded ${
                  loginProcessing
                    ? "bg-yellow-200 dark:bg-yellow-800"
                    : loginSuccess
                    ? "bg-green-200 dark:bg-green-800"
                    : error
                    ? "bg-red-200 dark:bg-red-800"
                    : "bg-gray-200 dark:bg-gray-800"
                }`}
              >
                {loginProcessing
                  ? "Processing..."
                  : loginSuccess === true
                  ? "Success"
                  : error
                  ? `Error: ${error}`
                  : loginSuccess === false
                  ? "Failed"
                  : "Not attempted"}
              </span>
            </div>
            {shares && (
              <div className="md:col-span-2">
                <span className="font-semibold">
                  {`Shamir shares used (${shares.length} of 16):`}
                </span>{" "}
                <span className="font-mono bg-gray-200 dark:bg-gray-800 px-2 py-1 rounded break-all text-xs">
                  {shares.join(", ")}
                </span>
              </div>
            )}
            {publicKey && (
              <div className="md:col-span-2">
                <span className="font-semibold">Mina Public Key:</span>{" "}
                <span className="font-mono bg-gray-200 dark:bg-gray-800 px-2 py-1 rounded break-all text-xs">
                  {publicKey}
                </span>
                <button
                  onClick={signMessage}
                  className="ml-2 px-2 py-1 bg-blue-500 text-white rounded"
                >
                  Sign
                </button>
              </div>
            )}

            {signedMessage && (
              <div className="md:col-span-2">
                <span className="font-semibold">Signature:</span>{" "}
                <span className="font-mono bg-gray-200 dark:bg-gray-800 px-2 py-1 rounded break-all text-xs">
                  {signedMessage.signature}
                </span>
                <span className="font-semibold">Message:</span>{" "}
                <span className="font-mono bg-gray-200 dark:bg-gray-800 px-2 py-1 rounded break-all text-xs">
                  {signedMessage.msg.join(", ")}
                </span>
              </div>
            )}
          </div>
        </div>

        <div className="flex flex-col gap-4">
          <h2 className="text-lg font-semibold mb-2">Sui Wallets</h2>
          <div className="flex gap-4 items-center flex-col sm:flex-row">
            <button
              onClick={handlePhantomSuiClick}
              className="rounded-full border border-solid border-black/[.08] dark:border-white/[.145] transition-colors flex items-center justify-center hover:bg-[#f2f2f2] dark:hover:bg-[#1a1a1a] hover:border-transparent font-medium text-sm sm:text-base h-10 sm:h-12 px-4 sm:px-5 w-full sm:w-auto md:w-[158px]"
            >
              Phantom
            </button>
            <button
              onClick={handleSlushClick}
              className="rounded-full border border-solid border-black/[.08] dark:border-white/[.145] transition-colors flex items-center justify-center hover:bg-[#f2f2f2] dark:hover:bg-[#1a1a1a] hover:border-transparent font-medium text-sm sm:text-base h-10 sm:h-12 px-4 sm:px-5 w-full sm:w-auto md:w-[158px]"
            >
              Slush
            </button>
          </div>
        </div>

        <div className="flex flex-col gap-4">
          <h2 className="text-lg font-semibold mb-2">Solana Wallets</h2>
          <div className="flex gap-4 items-center flex-col sm:flex-row">
            <button
              onClick={handlePhantomSolanaClick}
              className="rounded-full border border-solid border-black/[.08] dark:border-white/[.145] transition-colors flex items-center justify-center hover:bg-[#f2f2f2] dark:hover:bg-[#1a1a1a] hover:border-transparent font-medium text-sm sm:text-base h-10 sm:h-12 px-4 sm:px-5 w-full sm:w-auto md:w-[158px]"
            >
              Phantom
            </button>
            <button
              onClick={handleSolflareClick}
              className="rounded-full border border-solid border-black/[.08] dark:border-white/[.145] transition-colors flex items-center justify-center hover:bg-[#f2f2f2] dark:hover:bg-[#1a1a1a] hover:border-transparent font-medium text-sm sm:text-base h-10 sm:h-12 px-4 sm:px-5 w-full sm:w-auto md:w-[158px]"
            >
              Solflare
            </button>
          </div>
        </div>
        <div className="flex flex-col gap-4">
          <h2 className="text-lg font-semibold mb-2">Ethereum Wallets</h2>
          <div className="flex gap-4 items-center flex-col sm:flex-row">
            <button
              onClick={handlePhantomEthereumClick}
              className="rounded-full border border-solid border-black/[.08] dark:border-white/[.145] transition-colors flex items-center justify-center hover:bg-[#f2f2f2] dark:hover:bg-[#1a1a1a] hover:border-transparent font-medium text-sm sm:text-base h-10 sm:h-12 px-4 sm:px-5 w-full sm:w-auto md:w-[158px]"
            >
              Phantom
            </button>
            <button
              onClick={handleMetaMaskClick}
              className="rounded-full border border-solid border-black/[.08] dark:border-white/[.145] transition-colors flex items-center justify-center hover:bg-[#f2f2f2] dark:hover:bg-[#1a1a1a] hover:border-transparent font-medium text-sm sm:text-base h-10 sm:h-12 px-4 sm:px-5 w-full sm:w-auto md:w-[158px]"
            >
              MetaMask
            </button>
          </div>
        </div>
        <AuthComponent
          socialLogin={socialLogin}
          getPrivateKeyId={getPrivateKeyId}
          decryptShares={decryptShares}
          setPublicKey={setPublicKey}
        />
      </main>
    </div>
  );
}
