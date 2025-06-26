import { describe, it } from "node:test";
import assert from "node:assert";
import { checkZkappTransaction } from "o1js";
import { initBlockchain } from "@silvana-one/mina-utils";

const chain = process.env.CHAIN;

const expectedStatus = chain === "zeko" ? "pending" : "included";

const txHash = "5Ju44ifLx5rWSCJ93UW5iNUhdGGXSEk4fuV9xhmg36gtUaxXYYQw";

describe("Tx status", async () => {
  it("should get tx status", async () => {
    console.log({ chain });
    if (chain !== "devnet" && chain !== "zeko" && chain !== "local") {
      throw new Error("Invalid chain");
    }
    await initBlockchain(chain);
    const status = await checkZkappTransaction(txHash);
    console.log("status", status);
  });
});
