export async function getCurrentZekoSlot(
  chain: "zeko" | "zeko:alphanet" = "zeko"
): Promise<number | undefined> {
  try {
    const response = await fetch(
      "https://api.minascan.io/node/devnet/v1/graphql",
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          query: `
        query RuntimeConfig {
          runtimeConfig
        }
      `,
          variables: {},
        }),
      }
    );

    if (!response.ok) {
      console.error(
        `Failed to fetch runtime config: ${response.status} ${response.statusText} for chain ${chain}`
      );
      return undefined;
    }

    const {
      data: { runtimeConfig },
    } = await response.json();

    if (!runtimeConfig) {
      console.error(`No runtime config found for chain ${chain}`);
      return undefined;
    }

    if (
      !runtimeConfig.proof?.fork?.global_slot_since_genesis ||
      typeof runtimeConfig.proof.fork.global_slot_since_genesis !== "number"
    ) {
      console.error(`No fork slot found for chain ${chain}`);
      return undefined;
    }

    if (
      !runtimeConfig.genesis?.genesis_state_timestamp ||
      typeof runtimeConfig.genesis.genesis_state_timestamp !== "string"
    ) {
      console.error(`No genesis timestamp found for chain ${chain}`);
      return undefined;
    }

    const currentTimestamp = Date.now() / 1000;
    const forkSlot = runtimeConfig?.proof?.fork?.global_slot_since_genesis;
    const genesisTimestamp =
      Date.parse(runtimeConfig?.genesis?.genesis_state_timestamp) / 1000;

    // console.log("currentTimestamp", currentTimestamp);
    // console.log("forkSlot", forkSlot);
    // console.log("genesisTimestamp", genesisTimestamp);

    return Math.floor(forkSlot + (currentTimestamp - genesisTimestamp) / 180);
  } catch (error: any) {
    console.error(
      `Failed to fetch runtime config: ${
        error?.message ?? error
      } for chain ${chain}`
    );
    return undefined;
  }
}
