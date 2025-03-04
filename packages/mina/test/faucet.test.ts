import { describe, it } from "node:test";
import assert from "node:assert";
import { PrivateKey, PublicKey } from "o1js";
import { readFile, writeFile } from "node:fs/promises";
import {
  initBlockchain,
  accountBalanceMina,
  Zeko,
  Devnet,
} from "@silvana-one/mina-utils";
import { sleep } from "../src/sleep.js";
import { faucet, faucetDevnet } from "../src/faucet.js";

let keys: { privateKey: string; publicKey: string }[] = [];
const chain = process.env.CHAIN;
const MINIMUM_AMOUNT = 1000;
const keysToTopup: string[] = [];
const writeEnv = false;

describe("Topup", async () => {
  it("should check balances", async () => {
    console.log({ chain });
    if (chain !== "devnet" && chain !== "zeko") {
      throw new Error("Invalid chain");
    }
    await initBlockchain(chain);
    const keysJson = await readFile("./data/keys.json", "utf-8");
    const { keys: loadedKeys } = JSON.parse(keysJson) as {
      keys: { privateKey: string; publicKey: string }[];
    };
    keys = loadedKeys;
    if (writeEnv) {
      let envContent = "";
      for (let i = 0; i < keys.length; i++) {
        const { privateKey, publicKey } = keys[i];
        envContent += `# Account ${i + 1}\n`;
        envContent += `TEST_ACCOUNT_${i + 1}_PRIVATE_KEY=${privateKey}\n`;
        envContent += `TEST_ACCOUNT_${i + 1}_PUBLIC_KEY=${publicKey}\n\n`;
      }
      await writeFile("./data/.env", envContent);
    }
    for (let i = 0; i < keys.length; i++) {
      const { privateKey, publicKey } = keys[i];
      assert.strictEqual(
        PrivateKey.fromBase58(privateKey).toPublicKey().toBase58(),
        publicKey
      );
      const balance = await accountBalanceMina(PublicKey.fromBase58(publicKey));
      if (balance < MINIMUM_AMOUNT) {
        keysToTopup.push(publicKey);
      }
    }
    console.log({ keysToTopup });
  });
  it("should topup accounts on zeko", { skip: chain !== "zeko" }, async () => {
    for (const publicKey of keysToTopup) {
      const response = await faucet({
        publicKey,
        explorerUrl: Zeko.explorerAccountUrl ?? "",
        network: "devnet",
        faucetUrl: "https://zeko-faucet-a1ct.onrender.com/",
      });
      if (response.result !== "Successfully sent") {
        console.log("faucet error:", response);
        await sleep(180_000);
      }
      await sleep(5_000);
      const balance = await accountBalanceMina(PublicKey.fromBase58(publicKey));
      console.log(`${publicKey}: ${balance} MINA`);
    }
  });
  it(
    "should topup accounts on devnet",
    { skip: chain !== "devnet" },
    async () => {
      for (const publicKey of keysToTopup) {
        try {
          const response = await faucetDevnet({
            publicKey,
            explorerUrl: Devnet.explorerAccountUrl ?? "",
            network: "devnet",
            faucetUrl: "https://faucet.minaprotocol.com/api/v1/faucet",
          });
          console.log(`${publicKey}:`, response?.result?.status);
        } catch (e) {
          console.log(e);
          await sleep(120000);
        }
        await sleep(60000);
      }
    }
  );
});
