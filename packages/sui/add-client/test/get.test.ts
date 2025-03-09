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
const keys: string[] = [
  process.env.SECRET_KEY_1!,
  process.env.SECRET_KEY_2!,
  process.env.SECRET_KEY_3!,
];

describe("Sui state test", async () => {
  it("should get state", async () => {
    const packageID = process.env.PACKAGE_ID;
    if (!packageID) {
      throw new Error("PACKAGE_ID is not set");
    }
    const objectID = process.env.OBJECT_ID;
    if (!objectID) {
      throw new Error("OBJECT_ID is not set");
    }

    console.time("getObject");
    const data = await suiClient.getObject({
      id: objectID,
      options: {
        showContent: true,
      },
    });
    console.timeEnd("getObject");

    //console.log("dex", (data?.data?.content as unknown as Dex)?.fields?.tokens);
    const state = (data?.data?.content as unknown as Dex)?.fields?.actionsState;
    console.log("state", state);

    const { address, secretKey, keypair, balance } = await getKey({
      network,
      secretKey: keys[0],
    });

    async function buildTx(objectID: string, packageID: string) {
      const tx = new Transaction();
      console.time("moveCall");
      tx.moveCall({
        package: packageID,
        module: "tokens",
        function: "get_state",
        arguments: [tx.object(objectID)],
      });
      console.timeEnd("moveCall");
      tx.setSender(address);
      // tx.setGasBudget(10_000_000);
      console.time("sign");
      const signedTx = await tx.sign({
        signer: keypair,
        client: suiClient,
      });
      console.timeEnd("sign");
      return signedTx;
    }

    async function executeTx(tx: SignatureWithBytes, i: number) {
      const executedTx = await suiClient.dryRunTransactionBlock({
        transactionBlock: tx.bytes,
      });
      return {
        events: (executedTx.events as SuiEvent[])?.[0]?.parsedJson as object,
      };
    }
    console.time("get state");
    console.time("build txs");
    const tx = await buildTx(objectID, packageID);
    console.timeEnd("build txs");
    console.time("execute tx");
    const executedTx = await executeTx(tx, 0);
    console.timeEnd("execute tx");
    console.timeEnd("get state");
    console.log("executedTx", executedTx);
  });
});
