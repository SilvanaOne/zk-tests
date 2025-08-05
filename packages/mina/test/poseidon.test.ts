import { describe, it } from "node:test";
import assert from "node:assert";
import { poseidon } from "../src/verify/hash.js";

describe("Poseidon", () => {
  it(`should hash`, async () => {
    const message = [1n, 2n, 3n];
    const hash = poseidon(message);
    console.log("hash:", hash); // 24619730558757750532171846435738270973938732743182802489305079455910969360336n
    console.log("hash hex:", "0x" + hash.toString(16)); // 0x366e46102b0976735ed1cc8820c7305822a448893fee8ceeb42a3012a4663fd0
  });
});
