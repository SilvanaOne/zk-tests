"use client";
import { getWallets } from "@mysten/wallet-standard";
import { useEffect, useState } from "react";
import { connectPhantom, signPhantomMessage } from "@/lib/phantom";
import { connectSolflare, signSolflareMessage } from "@/lib/solflare";
import { login } from "@/lib/login";
import { rust_add } from "@/lib/precompiles";

let nonce = 0;

function getMessage() {
  nonce++;
  return `Silvana TEE login`;
  // nonce: ${nonce}
  // ${new Date().toISOString().replace("Z", "")} UTC`;
}

export default function Home() {
  const [chain, setChain] = useState<string | undefined>(undefined);
  const [wallet, setWallet] = useState<string | undefined>(undefined);
  const [message, setMessage] = useState<string | undefined>(undefined);
  const [signature, setSignature] = useState<string | undefined>(undefined);
  const [address, setAddress] = useState<string | undefined>(undefined);
  const [seed, setSeed] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loginSuccess, setLoginSuccess] = useState<boolean | null>(null);
  const [shares, setShares] = useState<number[] | null | undefined>(null);

  useEffect(() => {
    if (chain && wallet && address && signature && message) {
      login({
        chain,
        wallet,
        address,
        signature,
        message,
      }).then((result) => {
        console.log("Login result", result);
        setSeed(result.seed);
        setError(result.error);
        setLoginSuccess(result.success);
        setShares(result.indexes);
      });
    }
  }, [chain, wallet, address, signature, message]);

  function clear() {
    setChain(undefined);
    setWallet(undefined);
    setMessage(undefined);
    setSignature(undefined);
    setAddress(undefined);
    setSeed(null);
    setError(null);
    setLoginSuccess(null);
    setShares(null);
  }

  const handlePhantomSuiClick = async () => {
    clear();
    console.log("Phantom Sui button clicked");
    setChain("sui");
    setWallet("Phantom");

    const address = await connectPhantom("sui");
    console.log("Sui Address", address);
    setAddress(address);

    const messageText = getMessage();
    setMessage(messageText);

    const signedMessage = await signPhantomMessage({
      chain: "sui",
      message: messageText,
      display: "utf8",
    });
    console.log("Sui Signed message", signedMessage);
    const publicKey = (signedMessage as any)?.publicKey?.toString();
    console.log("Sui Public key", publicKey);
    const signature = (signedMessage as any)?.signature?.toString("hex");
    console.log("Sui Signature", signature);
    setSignature(signature);
  };

  const handlePhantomSolanaClick = async () => {
    clear();
    console.log("Phantom Solana button clicked");
    setChain("solana");
    setWallet("Phantom");

    const address = await connectPhantom("solana");
    console.log("Address", address);
    setAddress(address);

    const messageText = getMessage();
    setMessage(messageText);

    const signedMessage = await signPhantomMessage({
      chain: "solana",
      message: messageText,
      display: "utf8",
    });
    console.log("Signed message", signedMessage);
    const publicKey = (signedMessage as any)?.publicKey?.toString();
    console.log("Public key", publicKey);
    const signature = (signedMessage as any)?.signature?.toString("hex");
    console.log("Signature", signature);
    setSignature(signature);
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

    const messageText = "Hello, world!";
    setMessage(messageText);

    const message = new TextEncoder().encode(messageText);
    console.log("Message bytes", message);
    const signedMessage = await wallet?.features[
      "sui:signPersonalMessage"
    ].signPersonalMessage({
      message,
      account: connected.accounts[0],
      chain: "sui:mainnet",
    });
    console.log("Slush Signed message", signedMessage);
    setSignature(signedMessage?.signature);
  };

  const handleSolflareClick = async () => {
    clear();
    console.log("Solflare button clicked");
    setChain("solana");
    setWallet("Solflare");

    const result = await rust_add(1, 2);
    console.log("Rust add result", result);

    const address = await connectSolflare();
    console.log("Address", address);
    setAddress(address);

    const messageText = getMessage();
    setMessage(messageText);

    const signedMessage = await signSolflareMessage({
      message: messageText,
    });
    console.log("Signed message", signedMessage);
    const publicKey = signedMessage?.publicKey;
    console.log("Public key", publicKey);
    const signature = signedMessage?.signature;
    console.log("Signature", signature);
    setSignature(signature);
  };

  return (
    <div className="min-h-screen p-8 pb-20 sm:p-20 font-[family-name:var(--font-geist-sans)]">
      <main className="flex flex-col gap-4 items-center sm:items-start">
        {/* Connection Status Display */}
        <div className="w-full max-w-4xl bg-gray-50 dark:bg-gray-900 p-6 rounded-lg border">
          <h2 className="text-xl font-bold mb-4">Connection Status</h2>
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
                  loginSuccess
                    ? "bg-green-200 dark:bg-green-800"
                    : error
                    ? "bg-red-200 dark:bg-red-800"
                    : "bg-gray-200 dark:bg-gray-800"
                }`}
              >
                {loginSuccess === true
                  ? "Success"
                  : error
                  ? `Error: ${error}`
                  : loginSuccess === false
                  ? "Failed"
                  : "Not attempted"}
              </span>
            </div>
            {loginSuccess && (
              <div className="md:col-span-2">
                <span className="font-semibold">Seed:</span>{" "}
                <span className="font-mono bg-gray-200 dark:bg-gray-800 px-2 py-1 rounded break-all text-xs">
                  {seed || "No seed"}
                </span>
              </div>
            )}
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
      </main>
    </div>
  );
}
