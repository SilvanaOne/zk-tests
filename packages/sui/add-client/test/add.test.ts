import { describe, it } from "node:test";
import assert from "node:assert";
import {
  CoinBalance,
  getFullnodeUrl,
  SuiClient,
  SuiEvent,
} from "@mysten/sui/client";
import { getFaucetHost, requestSuiFromFaucetV1 } from "@mysten/sui/faucet";
import { MIST_PER_SUI } from "@mysten/sui/utils";
import { Ed25519Keypair } from "@mysten/sui/keypairs/ed25519";
import { Secp256k1Keypair } from "@mysten/sui/keypairs/secp256k1";
import { Transaction, TransactionArgument } from "@mysten/sui/transactions";
import crypto from "node:crypto";
import secp256k1 from "secp256k1";
import { getKey } from "../src/key.js";
import { SignatureWithBytes } from "@mysten/sui/cryptography";

const network: "testnet" | "devnet" | "localnet" = process.env.CHAIN! as
  | "testnet"
  | "devnet"
  | "localnet";
const suiClient = new SuiClient({
  url: getFullnodeUrl(network),
});

const keys: string[] = [
  process.env.SECRET_KEY_1!,
  process.env.SECRET_KEY_2!,
  process.env.SECRET_KEY_3!,
];

describe("Sui test", async () => {
  it("should test sui txs", async () => {
    const packageID = process.env.PACKAGE_ID;
    if (!packageID) {
      throw new Error("PACKAGE_ID is not set");
    }
    const objectID = process.env.OBJECT_ID;
    if (!objectID) {
      throw new Error("OBJECT_ID is not set");
    }
    // for (let i = 0; i < keys.length; i++) {
    //   const { address, secretKey, keypair, balance } = await getKey({
    //     network,
    //     secretKey: keys[i],
    //   });
    // }

    const data = await suiClient.getObject({
      id: objectID,
      options: {
        showContent: true,
      },
    });
    interface Dex {
      fields: {
        tokens: {
          fields: {
            id: string;
            tokenId: number;
            name: string;
            address: string;
            values: number[];
            sum: number;
          };
        }[];
        actionsState: number;
      };
    }
    //console.log("dex", (data?.data?.content as unknown as Dex)?.fields?.tokens);
    const addresses = (
      data?.data?.content as unknown as Dex
    )?.fields?.tokens.map((token) => token.fields.address);
    console.log("addresses", addresses);

    const TX_NUMBER = 3;
    const executedTxs: Promise<{
      digest: string;
      events: object;
    }>[] = [];
    const txs: Promise<SignatureWithBytes>[] = [];

    async function buildTx(i: number, objectID: string, packageID: string) {
      const { address, secretKey, keypair, balance } = await getKey({
        network,
        secretKey: keys[i],
      });
      console.time(`tx build ${i}`);
      const tx = new Transaction();

      tx.moveCall({
        package: packageID,
        module: "tokens",
        function: "add",
        arguments: [
          tx.object(objectID),
          tx.object(addresses[i]),
          tx.pure.u64(7),
        ],
      });
      // tx.setGasPayment([{
      //   objectId: address,
      //   version: 0,
      //     digest: "",
      //   },
      // ]);

      console.timeEnd(`tx build ${i}`);
      tx.setSender(address);
      tx.setGasBudget(10_000_000);

      console.time(`tx sign ${i}`);
      const signedTx = await tx.sign({
        signer: keypair,
        client: suiClient,
      });
      console.timeEnd(`tx sign ${i}`);
      // const dryRun = await suiClient.devInspectTransactionBlock({
      //   sender: keypair.toSuiAddress(),
      //   transactionBlock: signedTx.bytes
      // });
      // dryRun.effects.gasUsed.computationCost
      const gasPrice = await suiClient.getReferenceGasPrice();
      return signedTx;
    }

    console.time("build txs");
    for (let i = 0; i < TX_NUMBER; i++) {
      txs.push(buildTx(i, objectID, packageID));
    }
    console.timeEnd("build txs");

    console.time("await sign txs");
    const signedTxs: SignatureWithBytes[] = [];
    for (let i = 0; i < TX_NUMBER; i++) {
      signedTxs.push(await txs[i]);
    }
    console.timeEnd("await sign txs");

    async function executeTx(tx: SignatureWithBytes, i: number) {
      console.time(`tx execute ${i}`);

      const executedTx = await suiClient.executeTransactionBlock({
        transactionBlock: tx.bytes,
        signature: tx.signature,
        options: {
          showEffects: true,
          showObjectChanges: true,
          showInput: true,
          showEvents: true,
          showBalanceChanges: true,
        },
      });
      console.timeEnd(`tx execute ${i}`);
      return {
        digest: executedTx.digest,
        events: (executedTx.events as SuiEvent[])?.[0]?.parsedJson as object,
      };
    }
    console.time("execute txs");
    for (let i = TX_NUMBER - 1; i >= 0; i--) {
      executedTxs.push(executeTx(signedTxs[i], i));
    }
    console.timeEnd("execute txs");

    const executedAwaitedTxs: {
      digest: string;
      events: object;
    }[] = [];
    console.time("await execute txs");
    for (let i = TX_NUMBER - 1; i >= 0; i--) {
      executedAwaitedTxs.push(await executedTxs[i]);
    }
    console.timeEnd("await execute txs");

    async function awaitTx(
      executedTx: {
        digest: string;
        events: object;
      },
      i: number
    ) {
      console.time(`await tx ${i}`);
      const tx = executedTx;
      console.timeEnd(`await tx ${i}`);
      console.time(`tx wait ${i}`);
      const txWaitResult = await suiClient.waitForTransaction({
        digest: tx.digest,
        options: {
          showEffects: true,
          showObjectChanges: true,
          showInput: true,
          showEvents: true,
          showBalanceChanges: true,
        },
      });
      console.timeEnd(`tx wait ${i}`);
      console.log(
        `events ${i} after wait`,
        (txWaitResult.events as SuiEvent[])?.[0]?.parsedJson
      );
      console.log(`events ${i} before wait`, tx.events);
    }

    const awaitTxs: Promise<void>[] = [];
    console.time("await txs");
    for (let i = 0; i < TX_NUMBER; i++) {
      awaitTxs.push(awaitTx(executedAwaitedTxs[i], i));
    }
    console.timeEnd("await txs");

    console.time("await all txs");
    await Promise.all(awaitTxs);
    console.timeEnd("await all txs");
  });
});
