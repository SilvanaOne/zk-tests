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

describe("Sui events test", async () => {
  it("should get events", async () => {
    const packageID = process.env.PACKAGE_ID;
    if (!packageID) {
      throw new Error("PACKAGE_ID is not set");
    }
    const objectID = process.env.OBJECT_ID;
    if (!objectID) {
      throw new Error("OBJECT_ID is not set");
    }

    console.time("queryEvents");
    const data = await suiClient.queryEvents({
      query: {
        MoveModule: {
          package: packageID,
          module: "tokens",
        },
      },
      limit: 100,
      order: "descending",
    });
    console.timeEnd("queryEvents");
    console.log("data", data.data);
  });
});
