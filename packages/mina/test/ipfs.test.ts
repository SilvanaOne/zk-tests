import { describe, it } from "node:test";
import assert from "node:assert";
import {
  pinJSON,
  serializeIndexedMap,
  IndexedMapSerialized,
} from "@silvana-one/storage";

describe("IPFS", async () => {
  it("should pin json", async () => {
    const info = await pinJSON({
      data: {
        name: "test",
        data: { test: "test" },
      },
    });
    console.log("ipfs info", info);
  });
});
