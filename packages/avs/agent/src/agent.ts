async function agent() {
  console.time("Agent runtime");
  console.log("Agent is running");
  console.log("Agent arguments:", process.argv);
  await sleep(60000);
  const result = "some result";
  console.log("Agent work result:", result);
  console.timeEnd("Agent runtime");
  return result;
}

async function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

agent().catch(console.error);
