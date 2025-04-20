import { describe, it } from "node:test";
import assert from "node:assert";
import * as api from "@silvana-one/api";
import { getZkAppTxsFromBlockBerry } from "@silvana-one/mina-utils";

type Chain = "zeko" | "devnet" | "mainnet";
const chain: Chain = "mainnet" as Chain;
const address = "B62qnvHBRpBo5TX8LNJh6d2s9C7EraNChtKCmZhqxWnJ185UhZHWsi2";
api.config({
  apiKey: process.env.MINATOKENS_API_KEY!,
  chain,
});

describe("Nonce", () => {
  it(`should get nonce`, async () => {
    const txs = await getZkAppTxsFromBlockBerry({
      account: address,
      chain: "mainnet",
      blockBerryApiKey: process.env.BLOCKBERRY_API!,
    });
    const nonces = txs.data.map((tx: any) => tx.nonce);
    //console.log(txs);
    console.log(
      "Nonces:",
      nonces.sort((a: number, b: number) => a - b)
    );
    console.log("Latest nonce:", Math.max(...nonces));
  });
});
