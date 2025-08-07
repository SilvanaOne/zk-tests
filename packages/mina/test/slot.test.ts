import { describe, it } from "node:test";
import assert from "node:assert";
import { fetchLastBlock } from "o1js";
import { initBlockchain } from "@silvana-one/mina-utils";

type Chain = "zeko" | "devnet" | "mainnet";
const chain: Chain = "devnet" as Chain;

describe("Slot", () => {
  it(`should get slot`, async () => {
    await initBlockchain(chain);
    const lastBlock = await fetchLastBlock();
    console.log("last slot", lastBlock.globalSlotSinceGenesis.toBigint());
  });
});
