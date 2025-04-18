import { describe, it } from "node:test";
import assert from "node:assert";
import { poseidon } from "../src/verify/hash.js";

describe("Poseidon", () => {
  it(`should hash`, async () => {
    const message = [1n, 2n];
    const hash = poseidon(message);
    console.log("hash:", hash);
  });
});
