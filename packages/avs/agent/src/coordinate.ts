import {
  CoinBalance,
  getFullnodeUrl,
  SuiClient,
  SuiEvent,
} from "@mysten/sui/client";
import { Ed25519Keypair } from "@mysten/sui/keypairs/ed25519";
import { Transaction, TransactionArgument } from "@mysten/sui/transactions";
// console.log("devnet", getFullnodeUrl("devnet"));
// console.log("testnet", getFullnodeUrl("testnet"));
// console.log("mainnet", getFullnodeUrl("mainnet"));
// const suiClient = new SuiClient({
//   url: getFullnodeUrl("testnet"),
// });
// const packageID =
//   "0x083b05207706164149ba6cc263d799408e7018a77d5c68b3ae1caa6d1b650d93";
// const requestObjectID =
//   "0x904a847618f0a6724e3a8894286310190c4e53aa81d8ac61ddd1f073c6881a15";
// const responseObjectID =
//   "0x3a1e97787ee327749bffcae1609f797617fb8b6d7eb6e4f86cef51460c14a150";

const suiClient = new SuiClient({
  url: getFullnodeUrl("devnet"),
});
const packageID =
  "0xa34907868de25ec7e2bbb8e22021a3e702eb408bf87ec2bc3141a4c6b498ca01";
const requestObjectID =
  "0xca06480ff08a05c51a0aa80e8d74a655533c879370004e9e59b56f81bcb4ba3f";
const responseObjectID =
  "0x0a1ed77d28c4aa78ecf871c8ac817eb4763d03ed39289f5123f374a4f9d31318";

export async function coordination(params: {
  key: string;
  agent: string;
  action: string;
  data: string;
  isRequest: boolean;
}) {
  const { agent, action, key, data, isRequest } = params;

  const keypair = Ed25519Keypair.fromSecretKey(key);
  const address = keypair.toSuiAddress();
  console.log("address", address);
  console.time("tx build");
  const tx = new Transaction();

  const args: TransactionArgument[] = [
    tx.object(isRequest ? requestObjectID : responseObjectID),
    tx.pure.string(agent),
    tx.pure.string(action),
    tx.pure.string(data),
  ];

  tx.moveCall({
    package: packageID,
    module: "agent",
    function: isRequest ? "agent_request" : "agent_response",
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
}

export async function fetchRequest(): Promise<{
  agent: string;
  action: string;
  request: string;
}> {
  const data = await suiClient.getObject({
    id: requestObjectID,
    options: {
      showContent: true,
    },
  });
  return (data?.data?.content as any)?.fields;
}

export async function fetchResponse(): Promise<{
  agent: string;
  action: string;
  result: string;
}> {
  const data = await suiClient.getObject({
    id: responseObjectID,
    options: {
      showContent: true,
    },
  });
  return (data?.data?.content as any)?.fields;
}
