import { describe, it } from "node:test";
import assert from "node:assert";
import {
  PrivateKey,
  PublicKey,
  Mina,
  AccountUpdate,
  UInt64,
  Field,
  fetchEvents,
  Bool,
  fetchLastBlock,
  fetchAccount,
} from "o1js";
import { readFile, writeFile } from "node:fs/promises";
import {
  initBlockchain,
  accountBalanceMina,
  accountBalance,
  Zeko,
  Devnet,
  fetchMinaAccount,
  sendTx,
} from "@silvana-one/mina-utils";
import { faucet } from "../src/faucet.js";
import { sleep } from "../src/sleep.js";
import { pushEvent, emptyEvents } from "../src/events/events.js";

const { TestPublicKey } = Mina;
type TestPublicKey = Mina.TestPublicKey;

const chain = process.env.CHAIN;
const PRIVATE_KEY = process.env.TEST_ACCOUNT_3_PRIVATE_KEY;
let sender: TestPublicKey;

const expectedStatus = chain === "zeko" ? "pending" : "included";
const DELAY = 1000;
const NUMBER_OF_ITERATIONS = 100;
let retries = 0;

function arraysEqual(a: number[], b: number[]) {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

describe("Based rollup", async () => {
  it("should init blockchain", async () => {
    console.log({ chain });
    if (chain !== "devnet" && chain !== "zeko" && chain !== "local") {
      throw new Error("Invalid chain");
    }
    if (!PRIVATE_KEY) {
      throw new Error("PRIVATE_KEY is not set");
    }
    const { keys } = await initBlockchain(
      chain,
      chain === "local" ? 1 : undefined
    );
    const keysJson = await readFile("./data/keys.json", "utf-8");
    const { keys: loadedKeys } = JSON.parse(keysJson) as {
      keys: { privateKey: string; publicKey: string }[];
    };
    sender =
      chain === "local" ? keys[0] : TestPublicKey.fromBase58(PRIVATE_KEY);
  });

  it(`should send tx`, async () => {
    if (chain !== "devnet" && chain !== "zeko" && chain !== "local") {
      throw new Error("Invalid chain");
    }
    await fetchMinaAccount({ publicKey: sender, force: true });
    console.log("sender", sender.toJSON());
    console.log("sender balance", await accountBalanceMina(sender));
    let nonce = Number(Mina.getAccount(sender).nonce.toBigint());

    for (let i = 0; i < NUMBER_OF_ITERATIONS; i++) {
      console.log("iteration", i);
      console.time("calculate");
      const input = Array.from({ length: 5 }, () =>
        Math.floor(Math.random() * 1000)
      );
      let events = emptyEvents();
      events = pushEvent(events, input.map(Field));
      console.timeEnd("calculate");
      console.time("prepared");
      const account = await fetchAccount({ publicKey: sender });
      const receiptChainHash = account.account?.receiptChainHash;
      if (!receiptChainHash) {
        throw new Error("Receipt chain hash not found");
      }
      console.log("receiptChainHash", receiptChainHash.toJSON());

      const tx = await Mina.transaction(
        { sender, fee: 100_000_000, memo: `event5_${i}`, nonce: nonce++ },
        async () => {
          const update = AccountUpdate.create(sender);
          update.body.events = events;
          // update.body.preconditions.account.receiptChainHash.isSome =
          //   Bool(true);
          // update.body.preconditions.account.receiptChainHash.value =
          //   receiptChainHash;
        }
      );
      console.timeEnd("prepared");

      console.time("signed");
      tx.sign([sender.key]);
      console.timeEnd("signed");

      console.time("send");
      let txSent = await tx.safeSend();
      while (txSent.status !== "pending") {
        console.log("txSent retry", txSent.hash, txSent.status, txSent.errors);
        await sleep(5000);
        txSent = await tx.safeSend();
        retries++;
      }
      console.timeEnd("send");
      console.log("txSent", txSent.hash, txSent.status);
    }
  });
});
