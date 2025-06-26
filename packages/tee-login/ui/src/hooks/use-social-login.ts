"use client";
import { signIn, signOut, useSession } from "next-auth/react";
import { useCallback } from "react";
import { WalletProvider } from "@/lib/types";

export function useSocialLogin() {
  const { update } = useSession();

  return useCallback(
    async (provider: WalletProvider) => {
      // Ensure any existing NextAuth session cookie is cleared so that we
      // always obtain a completely fresh OAuth response (new id_token etc.)
      await signOut({ redirect: false });

      const popup = window.open(
        "about:blank",
        `${provider}OAuth`,
        "popup=yes,width=500,height=600"
      );
      if (!popup) {
        alert("Please enable pop‑ups and try again.");
        return;
      }
      const handleMsg = (ev: MessageEvent) => {
        if (ev.origin !== window.location.origin) return;
        if (ev.data === `oauth-ok`) {
          popup.close();
          update();
          console.log("oauth-ok", provider);
          window.removeEventListener("message", handleMsg);
        }
      };
      window.addEventListener("message", handleMsg);

      const res = await signIn(provider, {
        redirect: false,
        callbackUrl: `${window.location.origin}/oauth/popup-done`,
      });

      if (res?.error) {
        popup.close();
        window.removeEventListener("message", handleMsg);
        throw new Error(res.error);
      }
      if (res?.url) {
        popup.location.href = res.url;
      }
    },
    [update]
  );
}
