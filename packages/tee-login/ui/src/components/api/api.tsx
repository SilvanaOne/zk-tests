"use client";
import { Logger } from "@logtail/next";
import { useRef, useEffect, useImperativeHandle, forwardRef } from "react";

const log = new Logger({
  source: "Api",
});

// ---------- types the parent will see ----------
export interface ApiFrameHandle {
  /** returns hex-encoded signature */
  signMessage(msg: bigint[], publicKey: string): Promise<string>;
  signPayment(payment: string, publicKey: string): Promise<string | undefined>;
  privateKeyId(): Promise<{ privateKeyId: string; publicKey: string }>;
  decryptShares(data: string[], privateKeyId: string): Promise<string>;
  verifyAttestation(attestation: string): Promise<string>;
}

// internal message shapes -------------
type PrivateKeyIdRequest = { id: string; type: "private_key_id" };
type DecryptSharesRequest = {
  id: string;
  type: "decrypt_shares";
  data: string[];
  privateKeyId: string;
};
type SignMessageRequest = {
  id: string;
  type: "sign_message";
  msg: bigint[];
  publicKey: string;
};
type SignPaymentRequest = {
  id: string;
  type: "sign_payment";
  payment: string;
  publicKey: string;
};
type VerifyAttestationRequest = {
  id: string;
  type: "verify_attestation";
  attestation: string;
};

export type ApiRequest =
  | SignMessageRequest
  | SignPaymentRequest
  | PrivateKeyIdRequest
  | DecryptSharesRequest
  | VerifyAttestationRequest;

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
type SignMessageResponse = { id: string; type: "sign_message"; value: string }; // hex
type SignPaymentResponse = {
  id: string;
  type: "sign_payment";
  value: string | undefined;
}; // hex
type VerifyAttestationResponse = {
  id: string;
  type: "verify_attestation";
  value: string;
};
type ErrorResponse = { id: string; type: "error"; reason: string };
type ReadyResponse = { type: "ready" };

export type ApiResponse =
  | PrivateKeyIdResponse
  | DecryptSharesResponse
  | SignMessageResponse
  | SignPaymentResponse
  | ErrorResponse
  | ReadyResponse
  | VerifyAttestationResponse;

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
    async function handler(ev: MessageEvent<ApiResponse>) {
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

      if (ev.data.type === "sign_message") {
        pendingRequest.resolve(ev.data.value);
      } else if (ev.data.type === "sign_payment") {
        pendingRequest.resolve(ev.data.value);
      } else if (ev.data.type === "private_key_id") {
        pendingRequest.resolve(ev.data.value);
      } else if (ev.data.type === "decrypt_shares") {
        pendingRequest.resolve(ev.data.value);
      } else if (ev.data.type === "verify_attestation") {
        pendingRequest.resolve(ev.data.value);
      } else if (ev.data.type === "error") {
        pendingRequest.reject(new Error(ev.data.reason));
      } else {
        log.error("Unknown response type", {
          ev,
        });
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
      signMessage(msg, publicKey) {
        return new Promise<string>((resolve, reject) => {
          // Wait for iframe to be ready
          const attemptSignMessage = () => {
            if (!isReady.current) {
              setTimeout(attemptSignMessage, 1000);
              return;
            }

            const id = uuid();
            const req: SignMessageRequest = {
              id,
              type: "sign_message",
              msg,
              publicKey,
            };

            pending.current[id] = { resolve, reject };

            // Add timeout for the request
            setTimeout(() => {
              if (pending.current[id]) {
                delete pending.current[id];
                log.error("signMessage Request timeout", {
                  id,
                });
                reject(new Error("Request timeout"));
              }
            }, 5000);

            if (!frameRef.current?.contentWindow) {
              log.error("signMessage Iframe not available", {
                id,
              });
              reject(new Error("Iframe not available"));
              return;
            }

            frameRef.current.contentWindow.postMessage(req, "*");
          };

          attemptSignMessage();
        });
      },
      signPayment(payment, publicKey) {
        return new Promise<string>((resolve, reject) => {
          // Wait for iframe to be ready
          const attemptSignPayment = () => {
            if (!isReady.current) {
              setTimeout(attemptSignPayment, 1000);
              return;
            }

            const id = uuid();
            const req: SignPaymentRequest = {
              id,
              type: "sign_payment",
              payment,
              publicKey,
            };

            pending.current[id] = { resolve, reject };

            // Add timeout for the request
            setTimeout(() => {
              if (pending.current[id]) {
                delete pending.current[id];
                log.error("signPayment Request timeout", {
                  id,
                });
                reject(new Error("Request timeout"));
              }
            }, 5000);

            if (!frameRef.current?.contentWindow) {
              log.error("signPayment Iframe not available", {
                id,
              });
              reject(new Error("Iframe not available"));
              return;
            }

            frameRef.current.contentWindow.postMessage(req, "*");
          };

          attemptSignPayment();
        });
      },
      privateKeyId() {
        return new Promise<{ privateKeyId: string; publicKey: string }>(
          (resolve, reject) => {
            // Wait for iframe to be ready
            const attemptPrivateKeyId = () => {
              if (!isReady.current) {
                setTimeout(attemptPrivateKeyId, 1000);
                return;
              }

              const id = uuid();
              const req: PrivateKeyIdRequest = { id, type: "private_key_id" };
              pending.current[id] = { resolve, reject };

              setTimeout(() => {
                if (pending.current[id]) {
                  delete pending.current[id];
                  log.error("privateKeyId Request timeout", {
                    id,
                  });
                  reject(new Error("Request timeout"));
                }
              }, 5000);

              if (!frameRef.current?.contentWindow) {
                log.error("privateKeyId Iframe not available", {
                  id,
                });
                reject(new Error("Iframe not available"));
                return;
              }

              frameRef.current.contentWindow.postMessage(req, "*");
            };

            attemptPrivateKeyId();
          }
        );
      },
      decryptShares(data, privateKeyId) {
        return new Promise<string>((resolve, reject) => {
          // Wait for iframe to be ready
          const attemptDecryptShares = () => {
            if (!isReady.current) {
              setTimeout(attemptDecryptShares, 1000);
              return;
            }

            const id = uuid();
            const req: DecryptSharesRequest = {
              id,
              type: "decrypt_shares",
              data,
              privateKeyId,
            };

            pending.current[id] = { resolve, reject };

            setTimeout(() => {
              if (pending.current[id]) {
                delete pending.current[id];
                log.error("decryptShares Request timeout", {
                  id,
                });
                reject(new Error("Request timeout"));
              }
            }, 10000);

            if (!frameRef.current?.contentWindow) {
              log.error("decryptShares Iframe not available", {
                id,
              });
              reject(new Error("Iframe not available"));
              return;
            }

            frameRef.current.contentWindow.postMessage(req, "*");
          };

          attemptDecryptShares();
        });
      },
      verifyAttestation(attestation) {
        return new Promise<string>((resolve, reject) => {
          // Wait for iframe to be ready
          const attemptVerifyAttestation = () => {
            if (!isReady.current) {
              setTimeout(attemptVerifyAttestation, 1000);
              return;
            }

            const id = uuid();
            console.log("verifyAttestation called", id);
            const req: VerifyAttestationRequest = {
              id,
              type: "verify_attestation",
              attestation,
            };
            pending.current[id] = { resolve, reject };
            setTimeout(() => {
              if (pending.current[id]) {
                delete pending.current[id];
                log.error("verifyAttestation Request timeout", {
                  id,
                });
                reject(new Error(`Request timeout verifyAttestation ${id}`));
              }
            }, 5000);
            if (!frameRef.current?.contentWindow) {
              log.error("verifyAttestation Iframe not available", {
                id,
              });
              reject(new Error("Iframe not available"));
              return;
            }
            frameRef.current.contentWindow.postMessage(req, "*");
          };

          attemptVerifyAttestation();
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
