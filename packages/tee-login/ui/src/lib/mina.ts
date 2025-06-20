"use server";

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
if (!apiKey) {
  throw new Error("MINATOKENS_JWT_KEY is not set");
}
config({
  apiKey,
  chain: "devnet",
});

export async function balance(address: string): Promise<number> {
  try {
    const reply = (
      await getTokenBalance({
        body: { address },
      })
    ).data;
    if (reply && reply.balance && typeof reply.balance === "number") {
      return reply.balance;
    }
    return 0;
  } catch (error: any) {
    console.error("Error getting balance", error?.message);
    return 0;
  }
}

export async function faucet(address: string): Promise<string | undefined> {
  try {
    const reply = (await faucetApi({ body: { address } })).data;
    if (reply && reply.hash && typeof reply.hash === "string") {
      return reply.hash;
    }
    return undefined;
  } catch (error: any) {
    console.error("Error getting funds in faucet", error?.message);
    return undefined;
  }
}

export async function getNonce(address: string): Promise<number | undefined> {
  try {
    const reply = (await getNonceApi({ body: { address } })).data;
    console.log("getNonce reply:", reply);
    if (reply && reply.nonce && typeof reply.nonce === "number") {
      return reply.nonce;
    }
    return 0;
  } catch (error: any) {
    console.error("Error getting nonce", error?.message);
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
}): Promise<string | undefined> {
  const { from, to, amount, fee, memo } = params;
  const nonce = await getNonce(from);
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
    console.error("Error signing payment", error?.message);
    return undefined;
  }
}

export async function broadcastPayment(params: {
  payment: string;
}): Promise<string | undefined> {
  try {
    const { payment } = params;
    const { data, signature } = JSON.parse(payment);
    const DEVNET_GRAPHQL = "https://api.minascan.io/node/devnet/v1/graphql"; //Use a public GraphQL to broadcast or your local one
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
      console.error(
        "Error broadcasting payment:",
        response.status,
        response.statusText
      );
      return undefined;
    }

    const json = await response.json();
    const txHash = json?.data?.sendPayment?.payment?.hash;
    if (txHash) {
      console.log("Payment broadcasted with tx hash:", txHash);
      return txHash;
    } else {
      console.error(
        "Error broadcasting payment:",
        JSON.stringify(json, null, 2)
      );
      return undefined;
    }
  } catch (error: any) {
    console.error("Error broadcasting payment", error?.message);
    return undefined;
  }
}
