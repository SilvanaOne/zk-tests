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
  "0xd455ccf742350b9cbe8e8f3203910959f312794de0bc8ca42476baf1f96860c1";
const objectID =
  "0xe8cc661bb9c4475b643916bcd243391ba1330bebb355bce0f21900c445d623d0";

export async function coordinate(params: {
  key: string;
  agent: string;
  action: string;
}) {
  const { agent, action, key } = params;

  const keypair = Ed25519Keypair.fromSecretKey(key);
  const address = keypair.toSuiAddress();
  console.log("address", address);
  console.time("tx build");
  const tx = new Transaction();

  const args: TransactionArgument[] = [
    tx.object(objectID),
    tx.pure.string(agent),
    tx.pure.string(action),
  ];

  tx.moveCall({
    package: packageID,
    module: "agent",
    function: "run_agent",
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

export async function fetchAgent() {
  const data = await suiClient.getObject({
    id: objectID,
    options: {
      showContent: true,
    },
  });
  return (data?.data?.content as any)?.fields;
}
