"use client";

import { useState, useRef } from "react";
import { connectMetaMask, signMetaMaskMessage } from "@/lib/metamask";
import { getMessage, login, LoginRequest } from "@/lib/login";

export default function Login() {
  const [isConnecting, setIsConnecting] = useState(false);
  const [isLoggingIn, setIsLoggingIn] = useState(false);
  const [address, setAddress] = useState<string | null>(null);
  const [loginSuccess, setLoginSuccess] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [publicKey, setPublicKey] = useState<string | null>(null);
  const [shares, setShares] = useState<number[] | null>(null);

  // Crypto functions (simplified for main page - in production you'd want proper key management)
  const getPrivateKeyId = async () => {
    // This is a simplified version - in production you'd want proper key derivation
    const privateKeyId = "demo-private-key-id";
    const publicKey = "demo-public-key";
    return { privateKeyId, publicKey };
  };

  const decryptShares = async (data: string[], privateKeyId: string) => {
    // This is a simplified version - in production you'd decrypt the actual shares
    console.log("Decrypting shares:", data, "with key:", privateKeyId);
    return "demo-decrypted-public-key";
  };

  const handleMetaMaskLogin = async () => {
    setIsConnecting(true);
    setError(null);
    setLoginSuccess(null);

    try {
      // Step 1: Connect to MetaMask
      console.log("Connecting to MetaMask...");
      const walletAddress = await connectMetaMask();

      if (!walletAddress) {
        setError("Failed to connect to MetaMask");
        setIsConnecting(false);
        return;
      }

      setAddress(walletAddress);
      console.log("Connected to MetaMask:", walletAddress);

      // Step 2: Get private key for encryption
      const pk_result = await getPrivateKeyId();
      console.log("Private key result:", pk_result);
      if (!pk_result) {
        setError("Failed to generate private key");
        setIsConnecting(false);
        return;
      }

      // Step 3: Generate login message
      const msgData = await getMessage({
        login_type: "wallet",
        chain: "ethereum",
        wallet: "MetaMask",
        address: walletAddress,
        publicKey: pk_result.publicKey,
      });

      if (!msgData) {
        setError("Failed to generate login message");
        setIsConnecting(false);
        return;
      }

      setIsConnecting(false);
      setIsLoggingIn(true);

      // Step 4: Sign the message with MetaMask
      console.log("Requesting signature for message:", msgData.request.message);
      const signedMessage = await signMetaMaskMessage({
        message: msgData.request.message,
        display: "utf8",
      });

      if (!signedMessage) {
        setError("User rejected the signature request");
        setIsLoggingIn(false);
        return;
      }

      console.log("Message signed successfully");

      // Step 5: Send login request to backend
      const loginRequest: LoginRequest = {
        ...msgData.request,
        signature: signedMessage,
      };

      console.log("Sending login request to backend...");
      const result = await login(loginRequest);

      if (result.success && result.data) {
        console.log("Login successful!");
        setLoginSuccess(true);
        setShares(result.indexes || null);

        // Decrypt the shares to get the public key
        const decryptedPublicKey = await decryptShares(
          result.data,
          pk_result.privateKeyId
        );

        if (decryptedPublicKey) {
          setPublicKey(decryptedPublicKey);
        }

        setError(null);
      } else {
        setLoginSuccess(false);
        setError(result.error || "Login failed");
      }
    } catch (error: any) {
      console.error("MetaMask login error:", error);
      setError(`Login error: ${error?.message || "Unknown error"}`);
      setLoginSuccess(false);
    }

    setIsLoggingIn(false);
  };

  const handleReset = () => {
    setAddress(null);
    setLoginSuccess(null);
    setError(null);
    setPublicKey(null);
    setShares(null);
    setIsConnecting(false);
    setIsLoggingIn(false);
  };

  return (
    <div className="flex items-center justify-center min-h-screen bg-gray-50">
      <div className="max-w-md w-full mx-4">
        <div className="bg-white rounded-lg shadow-lg p-8">
          <div className="text-center mb-8">
            <h1 className="text-3xl font-bold text-gray-900 mb-2">
              Silvana TEE Login
            </h1>
            <p className="text-gray-600">
              Connect your MetaMask wallet to access the TEE
            </p>
          </div>

          {/* Connection Status */}
          {address && (
            <div className="mb-6 p-4 bg-blue-50 border border-blue-200 rounded-lg">
              <h3 className="font-semibold text-blue-900 mb-2">
                Connected Wallet
              </h3>
              <p className="text-sm text-blue-700 font-mono break-all">
                {address}
              </p>
            </div>
          )}

          {/* Login Status */}
          {(loginSuccess !== null || error) && (
            <div className="mb-6 p-4 rounded-lg border">
              <h3 className="font-semibold mb-2">Login Status</h3>
              <div
                className={`inline-flex items-center px-3 py-1 rounded-full text-sm font-medium ${
                  isLoggingIn
                    ? "bg-yellow-100 text-yellow-800"
                    : loginSuccess
                    ? "bg-green-100 text-green-800"
                    : error
                    ? "bg-red-100 text-red-800"
                    : "bg-gray-100 text-gray-800"
                }`}
              >
                {isLoggingIn
                  ? "Processing..."
                  : loginSuccess
                  ? "Success"
                  : error
                  ? "Failed"
                  : "Ready"}
              </div>

              {error && <p className="mt-2 text-sm text-red-600">{error}</p>}

              {shares && (
                <div className="mt-3">
                  <p className="text-sm font-medium text-gray-700">
                    Shamir Shares Used ({shares.length} of 16):
                  </p>
                  <p className="text-xs text-gray-600 font-mono mt-1">
                    {shares.join(", ")}
                  </p>
                </div>
              )}

              {publicKey && (
                <div className="mt-3">
                  <p className="text-sm font-medium text-gray-700">
                    Mina Public Key:
                  </p>
                  <p className="text-xs text-gray-600 font-mono mt-1 break-all">
                    {publicKey}
                  </p>
                </div>
              )}
            </div>
          )}

          {/* Action Buttons */}
          <div className="space-y-4">
            {!address ? (
              <button
                onClick={handleMetaMaskLogin}
                disabled={isConnecting}
                className="w-full flex items-center justify-center px-4 py-3 border border-transparent text-base font-medium rounded-lg text-white bg-orange-600 hover:bg-orange-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-orange-500 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
              >
                {isConnecting ? (
                  <>
                    <svg
                      className="animate-spin -ml-1 mr-3 h-5 w-5 text-white"
                      xmlns="http://www.w3.org/2000/svg"
                      fill="none"
                      viewBox="0 0 24 24"
                    >
                      <circle
                        className="opacity-25"
                        cx="12"
                        cy="12"
                        r="10"
                        stroke="currentColor"
                        strokeWidth="4"
                      ></circle>
                      <path
                        className="opacity-75"
                        fill="currentColor"
                        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                      ></path>
                    </svg>
                    Connecting...
                  </>
                ) : (
                  <>
                    <svg
                      className="w-5 h-5 mr-2"
                      viewBox="0 0 24 24"
                      fill="currentColor"
                    >
                      <path d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z" />
                      <path d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" />
                      <path d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z" />
                      <path d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" />
                    </svg>
                    Connect MetaMask
                  </>
                )}
              </button>
            ) : loginSuccess ? (
              <div className="space-y-3">
                <div className="text-center">
                  <div className="inline-flex items-center justify-center w-16 h-16 bg-green-100 rounded-full mb-4">
                    <svg
                      className="w-8 h-8 text-green-600"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth="2"
                        d="M5 13l4 4L19 7"
                      ></path>
                    </svg>
                  </div>
                  <h3 className="text-lg font-semibold text-gray-900 mb-2">
                    Login Successful!
                  </h3>
                  <p className="text-gray-600">
                    You are now authenticated with the TEE
                  </p>
                </div>
                <button
                  onClick={handleReset}
                  className="w-full px-4 py-2 border border-gray-300 text-gray-700 rounded-lg hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-gray-500 transition-colors"
                >
                  Login with Different Wallet
                </button>
              </div>
            ) : (
              <div className="space-y-3">
                <button
                  onClick={handleMetaMaskLogin}
                  disabled={isLoggingIn}
                  className="w-full px-4 py-3 border border-transparent text-base font-medium rounded-lg text-white bg-orange-600 hover:bg-orange-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-orange-500 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                >
                  {isLoggingIn ? (
                    <>
                      <svg
                        className="animate-spin -ml-1 mr-3 h-5 w-5 text-white inline"
                        xmlns="http://www.w3.org/2000/svg"
                        fill="none"
                        viewBox="0 0 24 24"
                      >
                        <circle
                          className="opacity-25"
                          cx="12"
                          cy="12"
                          r="10"
                          stroke="currentColor"
                          strokeWidth="4"
                        ></circle>
                        <path
                          className="opacity-75"
                          fill="currentColor"
                          d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                        ></path>
                      </svg>
                      Signing & Logging In...
                    </>
                  ) : (
                    "Sign Message & Login"
                  )}
                </button>
                <button
                  onClick={handleReset}
                  className="w-full px-4 py-2 border border-gray-300 text-gray-700 rounded-lg hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-gray-500 transition-colors"
                >
                  Reset
                </button>
              </div>
            )}
          </div>

          {/* Help Text */}
          <div className="mt-8 text-center">
            <p className="text-sm text-gray-500">
              Don't have MetaMask?{" "}
              <a
                href="https://metamask.io/"
                target="_blank"
                rel="noopener noreferrer"
                className="text-orange-600 hover:text-orange-500 font-medium"
              >
                Install it here
              </a>
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
