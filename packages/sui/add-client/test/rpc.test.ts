import { describe, it } from "node:test";
import assert from "node:assert";

const url = "https://fullnode.testnet.sui.io";
const request = {
  jsonrpc: "2.0",
  id: 1,
  method: "sui_getObject",
  params: [
    "0x904a847618f0a6724e3a8894286310190c4e53aa81d8ac61ddd1f073c6881a15",
    {
      showContent: true,
    },
  ],
};

describe("RPC test", async () => {
  it("should get object data", async () => {
    const response = await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(request),
    });
    if (!response.ok) {
      console.error("Error response", response.statusText);
      throw new Error("Failed to fetch object data");
    }
    const data = await response.json();
    console.log("data", data?.result?.data?.content?.fields);
    /*
data {
  action: 'action9',
  id: {
    id: '0x904a847618f0a6724e3a8894286310190c4e53aa81d8ac61ddd1f073c6881a15'
  },
  name: 'testagent2',
  nonce: '9',
  request: 'action9 requested'
}
    */
  });
});
