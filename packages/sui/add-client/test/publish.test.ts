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
import { writeFile } from "node:fs/promises";
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

describe("Sui publish package", async () => {
  it("should publish package", async () => {
    const { execSync } = await import("child_process");
    let bytes: string | undefined = undefined;

    console.log("Running sui client publish command...");
    try {
      bytes = execSync(
        "sui client publish --verify-deps --serialize-unsigned-transaction ../dex",
        {
          encoding: "utf-8",
        }
      );
      //console.log("Command output:", bytes);
    } catch (error) {
      console.error("Error running command:", error);
      throw error;
    }
    if (!bytes) {
      throw new Error("BYTES is not set");
    }

    const { address, secretKey, keypair, balance } = await getKey({
      network,
      secretKey: keys[0],
    });

    async function buildTx(txBytes: string) {
      const tx = Transaction.from(txBytes);
      const paginatedCoins = await suiClient.getCoins({
        owner: address,
      });
      const coins = paginatedCoins.data.map((coin) => {
        return {
          objectId: coin.coinObjectId,
          version: coin.version,
          digest: coin.digest,
        };
      });
      //console.log("coins", coins);

      tx.setSender(address);
      tx.setGasOwner(address);
      tx.setGasPayment(coins);
      //console.log("tx", await tx.toJSON());
      tx.setGasBudget(100_000_000);
      console.time("sign");
      const signedTx = await tx.sign({
        signer: keypair,
        client: suiClient,
      });
      console.timeEnd("sign");
      //console.log("signedTx", signedTx);
      return signedTx;
    }

    async function executeTx(tx: SignatureWithBytes) {
      console.time(`tx execute`);
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
      console.timeEnd(`tx execute`);
      //console.log("executedTx", executedTx);
      let packageID = "";
      let objectID = "";
      executedTx.objectChanges?.map((change) => {
        if (change.type === "published") {
          packageID = change.packageId;
        } else if (
          change.type === "created" &&
          change.objectType.includes("tokens::DEX")
        ) {
          objectID = change.objectId;
        }
      });
      return {
        digest: executedTx.digest,
        events: (executedTx.events as SuiEvent[])?.[0]?.parsedJson as object,
        packageID,
        objectID,
      };
    }
    console.time("execute tx");
    const tx = await buildTx(bytes);
    const executedTx = await executeTx(tx);
    console.timeEnd("execute tx");
    console.log("executedTx", executedTx);
    // Save contract addresses to .env.contracts
    const envContent = `PACKAGE_ID=${executedTx.packageID}
OBJECT_ID=${executedTx.objectID}`;
    await writeFile(".env.contracts", envContent);
  });
});
