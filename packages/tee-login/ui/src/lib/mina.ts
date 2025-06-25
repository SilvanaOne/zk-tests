"use server";
import { Logger } from "@logtail/next";
const log = new Logger({
  source: "mina",
});

import {
  config,
  getTokenBalance,
  faucet as faucetApi,
  getNonce as getNonceApi,
} from "@silvana-one/api";
import Client from "mina-signer";

const client = new Client({
  network: "testnet",
});
const apiKey = process.env.MINATOKENS_JWT_KEY;

function setChain(chain: "zeko" | "devnet") {
  if (!apiKey) {
    throw new Error("MINATOKENS_JWT_KEY is not set");
  }
  config({
    apiKey,
    chain,
  });
}

export async function balance(params: {
  address: string;
  chain: "zeko" | "devnet";
}): Promise<number> {
  const { address, chain } = params;
  try {
    setChain(chain);
    const reply = (
      await getTokenBalance({
        body: { address },
      })
    )?.data;
    if (reply && reply.balance && typeof reply.balance === "number") {
      return reply.balance;
    }
    return 0;
  } catch (error: any) {
    log.error("Error getting balance", error?.message);
    return 0;
  }
}

export async function faucet(params: {
  address: string;
  chain: "zeko" | "devnet";
}): Promise<
  | {
      success: boolean;
      txHash: string;
    }
  | { success: false; error: string }
> {
  const { address, chain } = params;
  try {
    setChain(chain);
    const reply = (await faucetApi({ body: { address } }))?.data;
    if (
      reply &&
      reply.success &&
      reply.hash &&
      typeof reply.hash === "string"
    ) {
      return { success: true, txHash: reply.hash };
    }
    return { success: false, error: reply?.error ?? "Faucet request failed" };
  } catch (error: any) {
    log.error("faucet error", error);
    const serializedError = serializeError(error);
    return {
      success: false,
      error: `Error while getting funds in faucet: ${
        serializedError ?? "error E305"
      }`,
    };
  }
}

export async function getNonce(params: {
  address: string;
  chain: "zeko" | "devnet";
}): Promise<number | undefined> {
  const { address, chain } = params;
  try {
    setChain(chain);
    const reply = (await getNonceApi({ body: { address } }))?.data;
    log.info("getNonce reply:", reply);
    if (reply && reply.nonce && typeof reply.nonce === "number") {
      return reply.nonce;
    }
    return 0;
  } catch (error: any) {
    log.error("Error getting nonce", error?.message);
    return undefined;
  }
}

export interface PaymentParams {
  from: string;
  to: string;
  amount: string;
  fee: string;
  nonce: number;
  memo: string;
}

export async function preparePayment(params: {
  from: string;
  to: string;
  amount: bigint;
  fee: bigint;
  memo: string;
  chain: "zeko" | "devnet";
}): Promise<string | undefined> {
  const { from, to, amount, fee, memo, chain } = params;
  const nonce = await getNonce({ address: from, chain });
  if (nonce === undefined) {
    return undefined;
  }
  const paymentParams: PaymentParams = {
    from,
    to,
    amount: amount.toString(),
    fee: fee.toString(),
    nonce,
    memo,
  };
  return JSON.stringify(paymentParams);
}

export async function signPayment(params: {
  payment: string;
  privateKey: string;
}): Promise<string | undefined> {
  try {
    const { privateKey } = params;
    const payment: PaymentParams = JSON.parse(params.payment);
    return JSON.stringify(client.signPayment(payment, privateKey));
  } catch (error: any) {
    log.error("Error signing payment", error?.message);
    return undefined;
  }
}

export async function broadcastPayment(params: {
  payment: string;
  chain: "zeko" | "devnet";
}): Promise<
  | {
      success: boolean;
      txHash: string;
    }
  | { success: false; error: string }
> {
  try {
    const { payment, chain } = params;
    setChain(chain);
    const { data, signature } = JSON.parse(payment);
    const DEVNET_GRAPHQL =
      chain === "zeko"
        ? "https://devnet.zeko.io/graphql"
        : "https://api.minascan.io/node/devnet/v1/graphql"; //Use a public GraphQL to broadcast or your local one
    const SEND_PAYMENT = `
    mutation SendPayment($input: SendPaymentInput!, $signature: SignatureInput!) {
      sendPayment(input: $input, signature: $signature) {
        payment { hash }
      }
    }`;

    const response = await fetch(DEVNET_GRAPHQL, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        query: SEND_PAYMENT,
        variables: { input: data, signature },
      }),
    });
    if (!response.ok) {
      log.error("Error broadcasting payment:", {
        status: response.status,
        statusText: response.statusText,
      });
      return {
        success: false,
        error: `Error broadcasting payment: ${response.status} ${response.statusText}`,
      };
    }

    const json = await response.json();
    const txHash = json?.data?.sendPayment?.payment?.hash;
    if (txHash) {
      log.info("Payment broadcasted with tx hash:", txHash);
      return { success: true, txHash };
    } else {
      log.error("Error broadcasting payment:", {
        json: JSON.stringify(json, null, 2),
      });
      return {
        success: false,
        error: `Error broadcasting payment: ${serializeError(json)}`,
      };
    }
  } catch (error: any) {
    log.error("Error broadcasting payment", error);
    const serializedError = serializeError(error);
    return {
      success: false,
      error: `Error broadcasting payment: ${serializedError ?? "error E306"}`,
    };
  }
}

export async function explorerUrl(params: {
  chain: "zeko" | "devnet";
  txHash: string;
}): Promise<string> {
  const { chain, txHash } = params;
  return chain === "zeko"
    ? `https://zekoscan.io/testnet/tx/${txHash}`
    : `https://minascan.io/devnet/tx/${txHash}`;
}

/**
 * Helper function to serialize error objects to strings
 * Handles various error formats including nested objects
 */
function serializeError(error: any): string | undefined {
  log.info("serializeError: error", error);
  if (!error) return undefined;

  // If it's already a string, return it
  if (typeof error === "string") return error;

  // If it has a message property, use that
  if (error.message && typeof error.message === "string") {
    return error.message;
  }

  // If it has an error property, recursively serialize it
  if (error.error) {
    const serialized = serializeError(error.error);
    if (serialized) return serialized;
  }

  // If it's an object, try to stringify it
  if (typeof error === "object") {
    try {
      return JSON.stringify(error);
    } catch {
      // If JSON.stringify fails, fall back to toString
      return error.toString();
    }
  }

  // For other types, convert to string
  return String(error);
}
