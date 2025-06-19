"use client";

import { useRef, useEffect, useImperativeHandle, forwardRef } from "react";

// ---------- types the parent will see ----------
export interface ApiFrameHandle {
  /** returns hex-encoded signature */
  sign(msg: bigint[], publicKey: string): Promise<string>;
  privateKeyId(): Promise<{ privateKeyId: string; publicKey: string }>;
  decryptShares(data: string[], privateKeyId: string): Promise<string>;
}

// internal message shapes -------------
type PrivateKeyIdRequest = { id: string; type: "private_key_id" };
type DecryptSharesRequest = {
  id: string;
  type: "decrypt_shares";
  data: string[];
  privateKeyId: string;
};
type SignRequest = {
  id: string;
  type: "sign";
  msg: bigint[];
  publicKey: string;
};

export type ApiRequest =
  | SignRequest
  | PrivateKeyIdRequest
  | DecryptSharesRequest;

type PrivateKeyIdResponse = {
  id: string;
  type: "private_key_id";
  value: { privateKeyId: string; publicKey: string };
};
type DecryptSharesResponse = {
  id: string;
  type: "decrypt_shares";
  value: string;
};
type SignResponse = { id: string; type: "sign"; value: string }; // hex
type ErrorResponse = { id: string; type: "error"; reason: string };
type ReadyResponse = { type: "ready" };

export type ApiResponse =
  | PrivateKeyIdResponse
  | DecryptSharesResponse
  | SignResponse
  | ErrorResponse
  | ReadyResponse;

// helpers ---------------------------------------

function uuid() {
  return crypto.randomUUID();
}

// -------------- the component ------------------
export const Api = forwardRef<ApiFrameHandle>(function Api(_props, ref) {
  const frameRef = useRef<HTMLIFrameElement>(null);
  const isReady = useRef(false);

  // pending promises keyed by id
  const pending = useRef<
    Record<string, { resolve: (v: any) => void; reject: (e: Error) => void }>
  >({});

  // listen for replies
  useEffect(() => {
    function handler(ev: MessageEvent<ApiResponse>) {
      // For sandboxed iframes without allow-same-origin, we can't reliably check ev.source
      // Instead, we rely on the fact that only our iframe should be posting to this origin
      // and that we control the iframe content

      if (ev.data?.type === "ready") {
        isReady.current = true;
        return;
      }

      if (!ev.data || typeof ev.data !== "object" || !("id" in ev.data)) {
        return; // not our message format
      }

      const { id } = ev.data;
      const pendingRequest = pending.current[id];
      if (!pendingRequest) {
        return; // unknown / already handled
      }
      delete pending.current[id];

      if (ev.data.type === "sign") {
        pendingRequest.resolve(ev.data.value);
      } else if (ev.data.type === "private_key_id") {
        pendingRequest.resolve(ev.data.value);
      } else if (ev.data.type === "decrypt_shares") {
        pendingRequest.resolve(ev.data.value);
      } else if (ev.data.type === "error") {
        pendingRequest.reject(new Error(ev.data.reason));
      } else {
        pendingRequest.reject(new Error("Unknown response type"));
      }
    }

    window.addEventListener("message", handler);
    return () => window.removeEventListener("message", handler);
  }, []);

  // expose the sign() method to parent via ref
  useImperativeHandle(
    ref,
    (): ApiFrameHandle => ({
      sign(msg, publicKey) {
        return new Promise<string>((resolve, reject) => {
          // Wait for iframe to be ready
          const attemptSend = () => {
            if (!isReady.current) {
              setTimeout(attemptSend, 50);
              return;
            }

            const id = uuid();
            const req: SignRequest = {
              id,
              type: "sign",
              msg,
              publicKey,
            };

            pending.current[id] = { resolve, reject };

            // Add timeout for the request
            setTimeout(() => {
              if (pending.current[id]) {
                delete pending.current[id];
                reject(new Error("Request timeout"));
              }
            }, 5000);

            if (!frameRef.current?.contentWindow) {
              reject(new Error("Iframe not available"));
              return;
            }

            frameRef.current.contentWindow.postMessage(req, "*");
          };

          attemptSend();
        });
      },
      privateKeyId() {
        return new Promise<{ privateKeyId: string; publicKey: string }>(
          (resolve, reject) => {
            const id = uuid();
            const req: PrivateKeyIdRequest = { id, type: "private_key_id" };
            pending.current[id] = { resolve, reject };

            setTimeout(() => {
              if (pending.current[id]) {
                delete pending.current[id];
                reject(new Error("Request timeout"));
              }
            }, 5000);

            if (!frameRef.current?.contentWindow) {
              reject(new Error("Iframe not available"));
              return;
            }

            frameRef.current?.contentWindow?.postMessage(req, "*");
          }
        );
      },
      decryptShares(data, privateKeyId) {
        return new Promise<string>((resolve, reject) => {
          const id = uuid();
          console.log("decryptShares 5", data, privateKeyId);
          const req: DecryptSharesRequest = {
            id,
            type: "decrypt_shares",
            data,
            privateKeyId,
          };
          console.log("decryptShares 6", req);
          pending.current[id] = { resolve, reject };
          console.log("decryptShares 7", pending.current);

          setTimeout(() => {
            if (pending.current[id]) {
              delete pending.current[id];
              reject(new Error("Request timeout"));
            }
          }, 10000);

          if (!frameRef.current?.contentWindow) {
            reject(new Error("Iframe not available"));
            return;
          }

          frameRef.current?.contentWindow?.postMessage(req, "*");
          console.log("decryptShares 3", data, privateKeyId);
        });
      },
    }),
    []
  );

  return (
    <iframe
      ref={frameRef}
      id="apiFrame"
      src="/login-api/v1/index.html"
      sandbox="allow-scripts"
      style={{ width: 0, height: 0, border: "none" }} // hidden for production
    />
  );
});
