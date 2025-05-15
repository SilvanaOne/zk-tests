import { describe, it } from "node:test";
import assert from "node:assert";
import { getFullnodeUrl, SuiClient, SuiEvent } from "@mysten/sui/client";
import {
  Transaction,
  TransactionArgument,
  ParallelTransactionExecutor,
} from "@mysten/sui/transactions";
import { getKey } from "../src/key.js";

const net = "devnet";
const suiClient = new SuiClient({
  url: getFullnodeUrl(net),
});

const COUNT = 1000;
const EXECUTORS = 1000;
const KEY = 52_000_000;
let error_count = 0;
let pause = false;

const txs: string[] = [];

const packageID =
  "0xb857e52715494b5ce2b9f9d491562c457ae00c64e92eaf7037fc896680e9a39e";
const objectID =
  "0xa85ff0a12c36bb9e1a1f9746b3d2244d32ce58994833507dcaaa7c5603a17f7a";

describe("Table test", async () => {
  it("should add table elements", async () => {
    const promises = [];
    let length = 0;
    for (let i = 0; i < EXECUTORS; i++) {
      while (pause) {
        await sleep(Math.floor(10000 * Math.random()) + 1000);
      }
      console.log(`Running executor ${i}`);
      promises.push(run(i));
      await sleep(10000);
      const current_length = txs.length;
      const txs_number = current_length - length;
      console.log(`TPS: ${txs_number / 10} for ${current_length} txs`);
      length = current_length;
    }
    const start = Date.now();
    for (const promise of promises) {
      await promise;
    }
    const end = Date.now();
    const current_length = txs.length;
    const txs_number = current_length - length;
    console.log(
      `Final TPS: ${
        txs_number / ((end - start) / 1000)
      } for ${current_length} txs`
    );
    console.log(`Time taken: ${(end - start) / 1000} seconds`);
    console.log(`Txs number: ${txs.length}`);
    console.log(`Error count: ${error_count}`);
  });
});

async function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function run(iterator: number) {
  try {
    const { address, secretKey, keypair, balance } = await getKey({
      network: net,
    });

    if (!packageID) {
      throw new Error("PACKAGE_ID is not set");
    }

    const executor = new ParallelTransactionExecutor({
      client: suiClient,
      signer: keypair,
      initialCoinBalance: 100_000_000n,
      minimumCoinBalance: 10_000_000n,
      maxPoolSize: 50,
    });

    const start = Date.now();
    for (let i = 0; i < COUNT; i++) {
      try {
        while (pause) {
          await sleep(Math.floor(10000 * Math.random()) + 1000);
          await executor.waitForLastTransaction();
        }
        const tx = new Transaction();

        const args: TransactionArgument[] = [
          tx.object(objectID),
          tx.pure.u256(KEY + i + iterator * COUNT),
          tx.pure.u256(KEY + i),
        ];

        tx.moveCall({
          package: packageID,
          module: "table",
          function: "add",
          arguments: args,
        });

        tx.setSender(address);
        tx.setGasBudget(10_000_000);

        const result = await executor.executeTransaction(tx);
        //console.log(`${iterator} digest`, result.digest);
        txs.push(result.digest);
      } catch (error: any) {
        console.log(
          `\x1b[31mError for iterator ${iterator} count ${i}: ${error.message}\x1b[0m`
        );
        error_count++;
        pause = true;
        await sleep(10000);
        await executor.waitForLastTransaction();
        pause = false;
      }
    }
    await executor.waitForLastTransaction();
    const end = Date.now();
    console.log(
      `Time taken: ${
        (end - start) / COUNT
      } milliseconds per tx for iterator ${iterator}`
    );
  } catch (error: any) {
    console.log(
      `\x1b[31mError for iterator ${iterator}: ${error.message}\x1b[0m`
    );
    error_count++;
    pause = true;
    await sleep(60000);
    pause = false;
  }
}
