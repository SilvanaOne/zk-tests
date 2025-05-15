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
const suiClient = new SuiClient({
  url: getFullnodeUrl("localnet"),
});

describe("Sui test", async () => {
  it("should test sui txs", async () => {
    const { address, secretKey, keypair, balance } = await getKey({
      network: "localnet",
    });

    const packageID = process.env.PACKAGE_ID;
    if (!packageID) {
      throw new Error("PACKAGE_ID is not set");
    }

    console.time("tx build");
    const tx = new Transaction();

    tx.moveCall({
      target: `${packageID}::add::add_create_transfer`,
      arguments: [tx.pure.address(packageID)],
    });
    tx.setSender(address);
    tx.setGasBudget(10_000_000);

    console.timeEnd("tx build");

    console.time("tx execute");
    const result = await suiClient.signAndExecuteTransaction({
      signer: keypair,
      transaction: tx,
    });
    console.timeEnd("tx execute");
    console.log("tx result", result);

    console.time("tx wait");
    const txWaitResult = await suiClient.waitForTransaction({
      digest: result.digest,
      options: {
        showEffects: true,
        showObjectChanges: true,
        showInput: true,
        showEvents: true,
        showBalanceChanges: true,
      },
    });
    console.timeEnd("tx wait");
    console.log("tx wait result", txWaitResult);
    console.log("events", (txWaitResult.events as SuiEvent[])?.[0]?.parsedJson);
  });
});
