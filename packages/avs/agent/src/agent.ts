import { coordinate, fetchAgent } from "./coordinate.js";

async function agent() {
  console.time("Agent runtime");
  console.log("Agent is running");
  console.log("Agent arguments:", process.argv.length - 2);
  const key = process.argv[2];
  const agent = process.argv[3];
  const action = process.argv[4];
  console.log("Agent arguments:", key.length, agent, action);
  await coordinate({ key, agent, action });
  await sleep(10000);
  const agentData = await fetchAgent();
  console.log("Agent:", agentData);
  const config = await fetch("https://dex.silvana.dev/api/v1/config", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      version: "0.1.0",
    }),
  });
  if (!config.ok) {
    console.error("Cannot get config", config.status, config.statusText);
  } else {
    const configData = await config.json();
    console.log("Agent work result:", configData);
  }

  console.timeEnd("Agent runtime");
}

async function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

agent().catch(console.error);
