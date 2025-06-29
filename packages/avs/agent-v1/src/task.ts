import { coordination, fetchRequest, fetchResponse } from "./coordinate.js";

async function task() {
  const key = process.argv[2];
  const agent = process.argv[3];
  const action = process.argv[4];
  console.log("Task arguments:", key.length, agent, action);
  await coordination({
    key,
    agent,
    action,
    data: action + " requested",
    isRequest: true,
  });
  await sleep(2000);
  const request = await fetchRequest();
  console.log("Task request:", request);
}

async function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

task().catch(console.error);
