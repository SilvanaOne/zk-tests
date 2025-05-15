import { describe, it } from "node:test";
import assert from "node:assert";
import {
  CoinBalance,
  getFullnodeUrl,
  SuiClient,
  SuiEvent,
} from "@mysten/sui/client";
import { Ed25519Keypair } from "@mysten/sui/keypairs/ed25519";
import { Transaction, TransactionArgument } from "@mysten/sui/transactions";
const suiClient = new SuiClient({
  url: getFullnodeUrl("devnet"),
});

describe("Sui TEE", async () => {
  it("should update PCRs", async () => {
    const keypair = Ed25519Keypair.fromSecretKey(process.env.SUI_KEY!);
    const address = keypair.toSuiAddress();
    console.log("address", address);
    console.time("tx build");
    const tx = new Transaction();
    const packageId = process.env.ENCLAVE_PACKAGE_ID!;
    const configObject = process.env.ENCLAVE_CONFIG_OBJECT_ID!;
    const capObject = process.env.CAP_OBJECT_ID!;
    console.log("packageId", packageId);
    console.log("configObject", configObject);
    console.log("capObject", capObject);
    // const pcr0 = stringToBytes(process.env.PCR0!);
    // const pcr1 = stringToBytes(process.env.PCR1!);
    // const pcr2 = stringToBytes(process.env.PCR2!);
    // console.log("pcr0", pcr0);
    // console.log("pcr1", pcr1);
    // console.log("pcr2", pcr2);

    const args: TransactionArgument[] = [
      tx.object(configObject),
      tx.object(capObject),
      tx.pure.string("name"),
      // tx.pure.vector("u8", pcr0),
      // tx.pure.vector("u8", pcr1),
      // tx.pure.vector("u8", pcr2),
    ];

    tx.moveCall({
      package: packageId,
      module: "enclave",
      function: "update_name",
      arguments: args,
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

function stringToBytes(str: string): number[] {
  // Convert a hex string to an array of numbers (bytes)
  if (!str) return [];

  // Remove '0x' prefix if present
  const hexStr = str.startsWith("0x") ? str.slice(2) : str;

  // Ensure the string has an even length
  const paddedHex = hexStr.length % 2 === 0 ? hexStr : "0" + hexStr;

  const bytes: number[] = [];
  for (let i = 0; i < paddedHex.length; i += 2) {
    bytes.push(parseInt(paddedHex.substring(i, i + 2), 16));
  }

  return bytes;
}
