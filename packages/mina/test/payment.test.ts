import { describe, it } from "node:test";
import assert from "node:assert";
import Client from "mina-signer";
import { config, getNonce as getNonceApi } from "@silvana-one/api";

const apiKey = process.env.MINATOKENS_API_KEY;
if (!apiKey) {
  throw new Error("MINATOKENS_API_KEY is not set");
}
config({
  apiKey,
  chain: "devnet",
});

const client = new Client({
  network: "testnet",
});

describe("Payment", () => {
  it(`should pay`, async () => {
    const privateKey = process.env.TEST_ACCOUNT_1_PRIVATE_KEY!;
    assert(privateKey, "TEST_ACCOUNT_1_PRIVATE_KEY is not set");
    const sender = client.derivePublicKey(privateKey);
    console.log("sender:", sender);
    const recipient = process.env.TEST_ACCOUNT_2_PUBLIC_KEY!;
    assert(recipient, "TEST_ACCOUNT_2_PUBLIC_KEY is not set");
    console.log("recipient:", recipient);
    const amount = 1_000_000_000n;
    const fee = 100_000_000n;
    const memo = "Silvana TEE payment";
    const payment = await preparePayment({
      from: sender,
      to: recipient,
      amount,
      fee,
      memo,
    });
    assert(payment, "payment is not set");
    console.log("payment:", payment);
    const signedPayment = await signPayment({
      payment,
      privateKey,
    });
    assert(signedPayment, "signedPayment is not set");
    console.log("signedPayment:", signedPayment);
    const txHash = await broadcastPayment({
      payment: signedPayment,
    });
    assert(txHash, "txHash is not set");
    console.log("txHash:", txHash);
  });
});

export async function getNonce(address: string): Promise<number | undefined> {
  try {
    const reply = (await getNonceApi({ body: { address } })).data;
    if (reply && reply.nonce && typeof reply.nonce === "number") {
      return reply.nonce;
    }
    return undefined;
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
  if (!nonce) {
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
