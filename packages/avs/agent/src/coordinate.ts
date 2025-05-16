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
const packageID =
  "0xd7d00456c2a25de783593317af1ea9955ce98cfde16a36ec6cca785e81d9e90a";
const requestObjectID =
  "0x402eb0550a27eaad2911ff5bd898ee61e6406c1eeac9c66a75cf2fe94cab3136";
const responseObjectID =
  "0x78eacc190e40775b303f73720d7eca47a3b6c81ac3588402b2d4bd841feb2820";

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
