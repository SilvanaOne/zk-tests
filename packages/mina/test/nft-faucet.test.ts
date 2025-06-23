import { describe, it } from "node:test";
import assert from "node:assert";
import { PrivateKey, PublicKey, Mina, AccountUpdate, UInt64 } from "o1js";
import { readFile, writeFile } from "node:fs/promises";
import {
  initBlockchain,
  accountBalanceMina,
  accountBalance,
  Zeko,
  Devnet,
  fetchMinaAccount,
} from "@silvana-one/mina-utils";
import { sleep } from "../src/sleep.js";
import { faucet, faucetDevnet } from "../src/faucet.js";
import { GASTANKS } from "./helpers/env.js";

let keys: { privateKey: string; publicKey: string }[] = [];
const tanks: { privateKey: string; publicKey: string }[] = [];
const chain = process.env.CHAIN;
const MINIMUM_AMOUNT = 100;
const keysToTopup: string[] = [];
const MAX_KEYS_TO_TOPUP = 50;

describe("Topup", async () => {
  it("should check balances", async () => {
    console.log({ chain });
    if (chain !== "devnet" && chain !== "zeko") {
      throw new Error("Invalid chain");
    }
    await initBlockchain(chain);
    const privateKeys = GASTANKS.map((key) => PrivateKey.fromBase58(key));
    keys = privateKeys.map((privateKey) => ({
      privateKey: privateKey.toBase58(),
      publicKey: privateKey.toPublicKey().toBase58(),
    }));
    for (let i = 0; i < keys.length; i++) {
      const { privateKey, publicKey } = keys[i];
      assert.strictEqual(
        PrivateKey.fromBase58(privateKey).toPublicKey().toBase58(),
        publicKey
      );
      const balance = await accountBalanceMina(PublicKey.fromBase58(publicKey));
      if (balance < MINIMUM_AMOUNT && keysToTopup.length < MAX_KEYS_TO_TOPUP) {
        keysToTopup.push(publicKey);
        console.log(`${publicKey}: ${balance} MINA`);
      }
    }
    console.log(`Accounts to topup: ${keysToTopup.length}`);
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
      const length = keysToTopup.length;
      for (let i = 0; i < length; i++) {
        try {
          const privateKey = PrivateKey.random();
          const publicKey = privateKey.toPublicKey();

          const response = await faucetDevnet({
            publicKey: publicKey.toBase58(),
            explorerUrl: Devnet.explorerAccountUrl ?? "",
            network: "devnet",
            faucetUrl: "https://faucet.minaprotocol.com/api/v1/faucet",
          });
          console.log(`${publicKey.toBase58()}:`, response?.result?.status);
          tanks.push({
            privateKey: privateKey.toBase58(),
            publicKey: publicKey.toBase58(),
          });
          await sleep(60000);
          await topup();
        } catch (e) {
          console.log(e);
          await sleep(120000);
        }
      }
      while (keysToTopup.length > 0 && tanks.length > 0) {
        await topup();
        await sleep(60000);
      }
    }
  );
});

async function topup() {
  if (keysToTopup.length === 0) {
    return;
  }
  if (tanks.length === 0) {
    return;
  }
  for (let i = 0; i < tanks.length; i++) {
    try {
      const publicKey = PublicKey.fromBase58(tanks[i].publicKey);
      let balance = await accountBalance(publicKey);
      if (balance.toBigInt() < 100_000_000_000n) {
        break;
      }
      console.log(
        `Tank ${i}: ${publicKey.toBase58()}: ${
          balance.toBigInt() / 1_000_000_000n
        } MINA`
      );
      const sender = publicKey;
      const receiver = PublicKey.fromBase58(keysToTopup[0]);
      const deployer = PrivateKey.fromBase58(tanks[i].privateKey);
      const fee = UInt64.from(100_000_000);
      await fetchMinaAccount({
        publicKey: receiver,
        force: false,
      });
      const isNew = !Mina.hasAccount(receiver);
      const amount = balance.sub(
        fee.add(UInt64.from(isNew ? 1_000_000_000 : 0))
      );
      const transaction = await Mina.transaction({ sender, fee }, async () => {
        if (isNew) AccountUpdate.fundNewAccount(sender, 1);
        const senderUpdate = AccountUpdate.createSigned(sender);
        senderUpdate.send({
          to: receiver,
          amount,
        });
      });
      const txSent = await transaction.sign([deployer]).send();
      console.log(`Sent tx${i}: ${receiver.toBase58()}: ${txSent.hash}`);
      keysToTopup.shift();
      tanks.splice(i, 1);
      await sleep(5000);
    } catch (e) {
      console.log(e);
      await sleep(120000);
    }
  }
}
