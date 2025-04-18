import { describe, it } from "node:test";
import assert from "node:assert";
import * as api from "@silvana-one/api";
import { sleep } from "@silvana-one/mina-utils";

type Chain = "zeko" | "devnet" | "mainnet";
const chain: Chain = "mainnet" as Chain;
const address = "B62qnvHBRpBo5TX8LNJh6d2s9C7EraNChtKCmZhqxWnJ185UhZHWsi2";
api.config({
  apiKey: process.env.MINATOKENS_API_KEY!,
  chain,
});

describe("Nonce", () => {
  it(`should get nonce`, async () => {
    while (true) {
      console.time("getNonce");
      const nonce = await api.getNonce({ body: { address } });
      console.timeEnd("getNonce");
      console.log("nonce:", nonce?.data?.nonce);
      await sleep(60000);
    }
  });
});
