"use client";
import { signIn, signOut, useSession } from "next-auth/react";
import { useState } from "react";
import { getMessage, login, LoginRequest, LoginResponse } from "@/lib/login";

export type SocialLoginFunction = (params: {
  address: string;
  result: LoginResponse;
  provider: "google" | "github";
}) => void;

export function AuthComponent(props: {
  socialLogin: SocialLoginFunction;
  getPrivateKeyId: () => Promise<{
    privateKeyId: string;
    publicKey: string;
  } | null>;
  decryptShares: (
    data: string[],
    privateKeyId: string
  ) => Promise<string | null>;
  setPublicKey: (publicKey: string) => void;
}) {
  const { socialLogin, getPrivateKeyId, decryptShares, setPublicKey } = props;
  const { data: session, status } = useSession();
  const [loginProcessing, setLoginProcessing] = useState(false);
  const [loginSuccess, setLoginSuccess] = useState<boolean | null>(null);
  const [loginError, setLoginError] = useState<string | null>(null);
  const [shares, setShares] = useState<number[] | null>(null);

  const handleSocialLogin = async (provider: "google" | "github") => {
    if (!session) {
      setLoginError("No session available");
      return;
    }

    if (provider === "google" && (!session?.idToken || !session?.user?.email)) {
      setLoginError("No ID token or email available");
      return;
    }

    if (
      provider === "github" &&
      (!session?.accessToken || !session?.user?.email)
    ) {
      setLoginError("No access token or email available");
      return;
    }

    // Check if the session provider matches the intended provider
    if (session.provider !== provider) {
      setLoginError(
        `Session provider (${session.provider}) does not match intended provider (${provider})`
      );
      return;
    }

    setLoginProcessing(true);
    setLoginError(null);

    try {
      console.log(`Starting ${provider} login with ID token:`, {
        email: session.user.email,
        name: session.user.name,
        provider: session.provider,
        hasIdToken: !!session.idToken,
        hasAccessToken: !!session.accessToken,
      });

      const pk_result = await getPrivateKeyId();
      if (!pk_result) {
        setLoginError("No private key id");
        return;
      }

      // Use the existing getMessage function with social login type
      const msgData = await getMessage({
        login_type: "social",
        chain: provider,
        wallet: provider,
        address: session.user.email ?? provider,
        publicKey: pk_result.publicKey,
      });

      if (!msgData) {
        setLoginError("Failed to generate login message");
        setLoginProcessing(false);
        return;
      }

      // Use the ID token (JWT) as signature - this contains the verifiable claims
      const loginRequest: LoginRequest = {
        ...msgData.request,
        signature: (provider === "google"
          ? session.idToken
          : session.accessToken) as string, // OAuth ID JWT token as signature,
        public_key: pk_result.publicKey,
      };

      console.log(`Sending ${provider} login request with ID token:`, {
        login_type: loginRequest.login_type,
        chain: loginRequest.chain,
        wallet: loginRequest.wallet,
        address: loginRequest.address,
        hasSignature: !!loginRequest.signature,
        signatureType: `${provider} ID JWT`,
      });

      // Use the existing login function to send to Rust backend
      const result = await login(loginRequest);

      console.log(`${provider} login result:`, result);

      if (result.success && result.data) {
        setLoginSuccess(true);
        setPublicKey(pk_result.publicKey);
        const publicKey = await decryptShares(
          result.data,
          pk_result.privateKeyId
        );
        if (!publicKey) {
          setLoginError("Failed to decrypt shares");
          setLoginSuccess(false);
          return;
        }
        setShares(result.data.map((share) => parseInt(share)) || null);
        setLoginError(null);
        socialLogin({
          address: session.user.email ?? provider,
          result: {
            ...result,
            publicKey,
          },
          provider,
        });
      } else {
        setLoginSuccess(false);
        setLoginError(result.error || "Unknown error");
      }
    } catch (error: any) {
      console.error(`${provider} login error:`, error);
      setLoginError(`${provider} login error: ${error?.message}`);
      setLoginSuccess(false);
    }

    setLoginProcessing(false);
  };

  const handleSignOut = () => {
    // Reset all state when signing out
    setLoginProcessing(false);
    setLoginSuccess(null);
    setLoginError(null);
    setShares(null);
    signOut();
  };

  if (status === "loading") {
    return <div className="p-4">Loading authentication...</div>;
  }

  if (session) {
    const providerName =
      session.provider === "google"
        ? "Google"
        : session.provider === "github"
        ? "GitHub"
        : session.provider;
    const providerColor =
      session.provider === "google"
        ? "blue"
        : session.provider === "github"
        ? "gray"
        : "blue";

    return (
      <div className="p-4 border rounded-md">
        <h2 className="text-xl font-bold mb-4">{providerName} Social Login</h2>

        {/* User Info */}
        <div className="space-y-2 mb-4">
          <p>
            <strong>Provider:</strong> {providerName}
          </p>
          <p>
            <strong>Name:</strong> {session.user?.name}
          </p>
          <p>
            <strong>Email:</strong> {session.user?.email}
          </p>
          <p>
            <strong>Access Token:</strong>{" "}
            {session.accessToken ? "✅ Available" : "❌ Not available"}
          </p>
          <p>
            <strong>ID Token (JWT):</strong>{" "}
            {session.idToken ? "✅ Available" : "❌ Not available"}
          </p>
        </div>

        {/* Token Preview */}
        {session.idToken && (
          <div
            className={`mb-4 p-3 bg-${providerColor}-50 border border-${providerColor}-200 rounded`}
          >
            <h3 className={`font-bold text-${providerColor}-800 mb-2`}>
              {providerName} ID Token (JWT):
            </h3>
            <code
              className={`text-xs bg-${providerColor}-100 p-2 rounded block break-all`}
            >
              {session.idToken.substring(0, 100)}...
            </code>
            <p className={`text-xs text-${providerColor}-700 mt-1`}>
              This JWT contains verifiable claims (iss, aud, sub, exp, iat,
              email) for your Rust backend
            </p>
          </div>
        )}

        {/* Social Login Status */}
        <div className="mb-4 p-3 rounded border">
          <h3 className="font-bold mb-2">TEE Login Status:</h3>
          <div className="space-y-2">
            <p>
              <strong>Processing:</strong> {loginProcessing ? "Yes" : "No"}
            </p>
            <p>
              <strong>Status:</strong>
              <span
                className={`ml-2 px-2 py-1 rounded text-sm ${
                  loginProcessing
                    ? "bg-yellow-200 text-yellow-800"
                    : loginSuccess === true
                    ? "bg-green-200 text-green-800"
                    : loginSuccess === false
                    ? "bg-red-200 text-red-800"
                    : "bg-gray-200 text-gray-800"
                }`}
              >
                {loginProcessing
                  ? "Processing..."
                  : loginSuccess === true
                  ? "Success"
                  : loginSuccess === false
                  ? "Failed"
                  : "Ready"}
              </span>
            </p>
            {loginError && (
              <p>
                <strong>Error:</strong>{" "}
                <span className="text-red-600">{loginError}</span>
              </p>
            )}
            {shares && (
              <p>
                <strong>Shares Used:</strong> {shares.join(", ")} (
                {shares.length} of 16)
              </p>
            )}
          </div>
        </div>

        {/* Request Details */}
        <div className="mb-4 p-3 bg-blue-50 border border-blue-200 rounded">
          <h3 className="font-bold text-blue-800 mb-2">
            Will Send to Rust Backend:
          </h3>
          <div className="text-sm space-y-1">
            <p>
              <strong>login_type:</strong> <code>social</code>
            </p>
            <p>
              <strong>chain:</strong> <code>{session.provider}</code>
            </p>
            <p>
              <strong>wallet:</strong> <code>{session.provider}</code>
            </p>
            <p>
              <strong>address:</strong> <code>{session.user?.email}</code>
            </p>
            <p>
              <strong>signature:</strong>{" "}
              <code className="text-xs">
                {providerName} ID JWT token (verifiable)
              </code>
            </p>
          </div>
        </div>

        {/* Actions */}
        <div className="space-x-2">
          <button
            onClick={() =>
              handleSocialLogin(session.provider as "google" | "github")
            }
            disabled={
              loginProcessing ||
              (session.provider === "google" && !session.idToken) ||
              (session.provider === "github" && !session.accessToken)
            }
            className="bg-green-500 text-white px-4 py-2 rounded hover:bg-green-600 disabled:opacity-50"
          >
            {loginProcessing ? "Sending..." : "Send to Rust Backend"}
          </button>

          <button
            onClick={handleSignOut}
            className="bg-red-500 text-white px-4 py-2 rounded hover:bg-red-600"
          >
            Sign Out
          </button>

          {loginSuccess === false && (
            <button
              onClick={() =>
                handleSocialLogin(session.provider as "google" | "github")
              }
              disabled={loginProcessing}
              className="bg-blue-500 text-white px-4 py-2 rounded hover:bg-blue-600 disabled:opacity-50"
            >
              {loginProcessing ? "Retrying..." : "Retry"}
            </button>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="p-4 border rounded-md">
      <h2 className="text-xl font-bold mb-4">Social Login</h2>
      <p className="mb-4 text-gray-600">
        Sign in with Google or GitHub to get ID JWT token for TEE backend
        verification
      </p>
      <div className="space-x-2">
        <button
          onClick={() => signIn("google")}
          className="bg-blue-500 text-white px-4 py-2 rounded hover:bg-blue-600"
        >
          Sign In with Google
        </button>
        <button
          onClick={() => signIn("github")}
          className="bg-gray-800 text-white px-4 py-2 rounded hover:bg-gray-900"
        >
          Sign In with GitHub
        </button>
      </div>
    </div>
  );
}
