"use server";

import { config, getTokenBalance, faucet as faucetApi } from "@silvana-one/api";

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
