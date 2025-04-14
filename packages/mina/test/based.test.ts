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
import { sleep } from "../src/sleep.js";
import { pushEvent, emptyEvents } from "../src/events/events.js";

const { TestPublicKey } = Mina;
type TestPublicKey = Mina.TestPublicKey;

const chain = process.env.CHAIN;
const PRIVATE_KEY = process.env.TEST_ACCOUNT_1_PRIVATE_KEY;
let sender: TestPublicKey;

const expectedStatus = chain === "zeko" ? "pending" : "included";
const DELAY = 1000;
const NUMBER_OF_ITERATIONS = 50;

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

      const tx = await Mina.transaction(
        { sender, fee: 100_000_000, memo: `event ${i}`, nonce: nonce++ },
        async () => {
          const update = AccountUpdate.create(sender);
          update.body.events = events;
        }
      );
      console.timeEnd("prepared");

      console.time("signed");
      tx.sign([sender.key]);
      console.timeEnd("signed");

      console.time("send");
      let txSent = await tx.safeSend();
      while (txSent.status !== "pending") {
        await sleep(5000);
        txSent = await tx.safeSend();
      }
      console.timeEnd("send");
      console.log("txSent", txSent.hash, txSent.status);

      //await sleep(DELAY);
      let fetched = false;
      let attempts = 0;
      console.time("fetch");
      while (!fetched) {
        const fetchedEvents = await fetchEvents({
          publicKey: sender.toBase58(),
        });

        const data =
          fetchedEvents[fetchedEvents.length - 1]?.events[0]?.data.map(Number);
        //console.log("fetchedEvents", data);
        //assert.deepStrictEqual(data, input);
        fetched = arraysEqual(data, input);
        attempts++;
        if (attempts > 100) {
          console.timeEnd("fetch");
          throw new Error("Failed to fetch events");
        }
      }
      console.timeEnd("fetch");
      if (attempts > 1) {
        console.log("attempts", attempts);
      }
    }
  });
});
