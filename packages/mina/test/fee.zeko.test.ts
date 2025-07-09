import { describe, it } from "node:test";
import assert from "node:assert";
import { sleep } from "../src/sleep.js";
import { fetchZekoFee } from "../src/zeko-fee.js";

const MAX_FEE = 50_000_000n;

describe("Zeko fee", async () => {
  it(`should wait for low fee`, async () => {
    while (true) {
      const fee = await fetchZekoFee({ txn: 2 });

      if (fee && fee.toBigInt() < MAX_FEE) {
        console.log(
          "fee:",
          Number((fee?.toBigInt() * 1000n) / 1_000_000_000n) / 1000
        );
        break;
      } else if (fee) {
        console.log(
          "fee is too high",
          Number((fee?.toBigInt() * 1000n) / 1_000_000_000n) / 1000
        );
      } else {
        console.log("fee is undefined");
      }
      await sleep(10000);
    }
  });
});
