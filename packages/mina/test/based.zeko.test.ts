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
import { fetchZekoFee } from "../src/zeko-fee.js";
import { formatTime } from "../src/time.js";

const MAX_FEE = 1000n;
const NUMBER_OF_ITERATIONS = 1000;

const { TestPublicKey } = Mina;
type TestPublicKey = Mina.TestPublicKey;

const chain = process.env.CHAIN;
const PRIVATE_KEY = process.env.TEST_ACCOUNT_3_PRIVATE_KEY;
let sender: TestPublicKey;

const expectedStatus = chain === "zeko" ? "pending" : "included";
const DELAY = 1000;

let retries = 0;

function arraysEqual(a: number[], b: number[]) {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

describe("Based rollup", async () => {
  it.skip("should init blockchain", async () => {
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
  it.skip("should topup accounts on zeko", async () => {
    if (!PRIVATE_KEY) {
      throw new Error("PRIVATE_KEY is not set");
    }
    sender = TestPublicKey.random();
    console.log("sender", sender.toBase58());
    const response = await faucet({
      publicKey: sender.toBase58(),
      explorerUrl: Zeko.explorerAccountUrl ?? "",
      network: "devnet",
      faucetUrl: "https://zeko-faucet-a1ct.onrender.com/",
    });
    if (response.result !== "Successfully sent") {
      console.log("faucet error:", response);
    } else {
      console.log("faucet success:", response.result);
    }
    const url = "https://devnet.zeko.io/graphql"; // "http://m1.zeko.io/graphql";

    const networkInstance = Mina.Network({
      mina: url,
      archive: url,
      networkId: "testnet",
    });
    Mina.setActiveInstance(networkInstance);
    const balance = await accountBalanceMina(sender);
    console.log(`${sender.toBase58()}: ${balance} MINA`);
  });
  it("should init zeko alphanet", async () => {
    console.log({ chain });
    if (chain !== "zeko") {
      throw new Error("Invalid chain, should be zeko");
    }
    if (!PRIVATE_KEY) {
      throw new Error("PRIVATE_KEY is not set");
    }

    const url = "https://devnet.zeko.io/graphql";
    //const url = "http://m1.zeko.io/graphql";

    const networkInstance = Mina.Network({
      mina: url,
      archive: url,
      networkId: "testnet",
    });
    Mina.setActiveInstance(networkInstance);
    // const info = Mina.getNetworkConstants();
    // console.log(
    //   "genesisTimestamp",
    //   new Date(Number(info.genesisTimestamp.toBigInt()))
    // );
    // console.log("accountCreationFee", info.accountCreationFee.toBigInt()); // 1 000 000 000n

    sender = TestPublicKey.fromBase58(PRIVATE_KEY);
    console.log("sender", sender.toBase58());
    const balance = await accountBalanceMina(sender);
    console.log(`${sender.toBase58()}: ${balance} MINA`);
    const kvBalance = await accountBalance(
      PublicKey.fromBase58(
        "B62qo69VLUPMXEC6AFWRgjdTEGsA3xKvqeU5CgYm3jAbBJL7dTvaQkv"
      )
    );
    console.log(`kvBalance: ${kvBalance} MINA`);
    // const kvAccount = await fetchAccount({
    //   publicKey: PublicKey.fromBase58(
    //     "B62qo69VLUPMXEC6AFWRgjdTEGsA3xKvqeU5CgYm3jAbBJL7dTvaQkv"
    //   ),
    // });
    // console.log("kvAccount", kvAccount);
  });
  it(`should send tx`, async () => {
    if (chain !== "devnet" && chain !== "zeko" && chain !== "local") {
      throw new Error("Invalid chain");
    }
    await fetchMinaAccount({ publicKey: sender, force: true });
    console.log("sender", sender.toJSON());
    console.log("sender balance", await accountBalanceMina(sender));
    let nonce = Number(Mina.getAccount(sender).nonce.toBigint());
    const start = Date.now();

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
        { sender, fee: 100_000_000, memo: `event3_${i}`, nonce: nonce++ },
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
      const fee = await fetchZekoFee({ txn: tx });

      if (fee && fee.toBigInt() < MAX_FEE * 1_000_000_000n) {
        console.log(
          "fee:",
          Number((fee?.toBigInt() * 1000n) / 1_000_000_000n) / 1000
        );
        tx.setFee(fee);
      }

      console.time("signed");
      tx.sign([sender.key]);
      console.timeEnd("signed");

      console.time("send");
      let txSent = await tx.safeSend();
      while (txSent.status !== "pending") {
        console.log("txSent retry", txSent.hash, txSent.status, txSent.errors);
        await sleep(5000);
        const fee = await fetchZekoFee({ txn: tx });
        if (fee) {
          console.log(
            "fee:",
            Number((fee?.toBigInt() * 1000n) / 1_000_000_000n) / 1000
          );
        }
        if (fee && fee.toBigInt() < MAX_FEE * 1_000_000_000n) {
          tx.setFee(fee);
          tx.sign([sender.key]);
        }

        txSent = await tx.safeSend();
        retries++;
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
      console.log("retries", retries);
    }
    const end = Date.now();
    console.log(
      `time for ${NUMBER_OF_ITERATIONS} iterations`,
      formatTime(end - start)
    );
    console.log(
      `time per iteration`,
      formatTime((end - start) / Number(NUMBER_OF_ITERATIONS))
    );
    console.log(
      `txs per minute`,
      (NUMBER_OF_ITERATIONS * 60000) / (end - start)
    );
  });
});
