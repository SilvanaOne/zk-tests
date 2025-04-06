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

const suiClient = new SuiClient({
  url: getFullnodeUrl("localnet"),
});

/*
Status
Newly created
Blob ID pxwNUDlb_goHoq81w3ElVTo_DnxahDrBlbkn2a90kpQ
Associated Sui Object 0xd98a87cf0012119fb7571935b5e1cb235cca1f30a9b702b8d99abd45df86c7da
Stored until epoch 27


Path: nft.json
Blob ID: fTXNwlaxLxivxwgZ32eTjXWe7OoBlsZVVubOS7dDt6I
Sui object ID: 0xd8bb3557e066d1a4c7d064456a9e7c7f81f2b29e757dd258c8e41345563527ca
Unencoded size: 525 B
Encoded size (including replicated metadata): 63.0 MiB
Cost (excluding gas): 0.0001 WAL (storage was purchased, and a new blob object was registered) 
Expiry epoch (exclusive): 27
Encoding type: RedStuff/Reed-Solomon

info {
  newlyCreated: {
    blobObject: {
      id: '0x36ffb847a3bef62a9949962084afc862b1db677203b15c23c3613a4ece10c0ae',
      registeredEpoch: 26,
      blobId: '5XJtqFdsUn3jRiPYVR7VA1Uko-46zKja-f95eFajsFw',
      size: 102,
      encodingType: 'RS2',
      certifiedEpoch: 26,
      storage: [Object],
      deletable: false
    },
    resourceOperation: { registerFromScratch: [Object] },
    cost: 132300
  }
}


https://aggregator.walrus-testnet.walrus.space/v1/blobs/5XJtqFdsUn3jRiPYVR7VA1Uko-46zKja-f95eFajsFw

*/

const address =
  "0xc3af09dbd444d54bc9ca64fb7f8cb1b95cb63c8d51eac3227096bb8f049166da";
const SUI_NETWORK = "testnet";
const SUI_VIEW_TX_URL = `https://suiscan.xyz/${SUI_NETWORK}/tx`;
const SUI_VIEW_OBJECT_URL = `https://suiscan.xyz/${SUI_NETWORK}/object`;
const daemon: "local" | "testnet" = "testnet" as "local" | "testnet";
const basePublisherUrl =
  daemon === "local"
    ? "http://127.0.0.1:31415"
    : "https://wal-publisher-testnet.staketab.org"; //"https://publisher.walrus-testnet.walrus.space";
const readerUrl =
  daemon === "local"
    ? "http://127.0.0.1:31415/v1/blobs/"
    : "https://wal-aggregator-testnet.staketab.org/v1/blobs/"; //"https://aggregator.walrus-testnet.walrus.space/v1/blobs/";
let blobId = "5XJtqFdsUn3jRiPYVR7VA1Uko-46zKja-f95eFajsFw";
const numEpochs = 100;
let text = "Hello, world!";

describe("Walrus test", async () => {
  it("should test walrus", async () => {
    let sendToParam = `&send_object_to=${address}`;
    console.log("Writing to Walrus");
    console.time("written");
    const response = await fetch(
      `${basePublisherUrl}/v1/blobs?epochs=${numEpochs}${sendToParam}`,
      {
        method: "PUT",
        body: text,
      }
    );
    console.timeEnd("written");
    if (response.status === 200) {
      const info = await response.json();
      console.log("info", info);
      blobId =
        info?.newlyCreated?.blobObject?.blobId ??
        info?.alreadyCertified?.blobId;
      console.log("blobId", blobId);
    } else {
      console.log("response.statusText", {
        statusText: response.statusText,
        status: response.status,
      });
    }
  });
  it("should read from walrus", async () => {
    if (!blobId) {
      throw new Error("blobId is not set");
    }
    console.log("Reading blob", blobId);
    console.time("read");
    const response = await fetch(`${readerUrl}${blobId}`);
    console.timeEnd("read");
    if (!response.ok) {
      console.log("response:", {
        statusText: response.statusText,
        status: response.status,
      });
    } else {
      const info = await response.json();
      console.log("info", info);
    }
  });
});
