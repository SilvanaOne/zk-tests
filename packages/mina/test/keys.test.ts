import { describe, it } from "node:test";
import assert from "node:assert";
import { PrivateKey, PublicKey } from "o1js";
import { writeFile } from "node:fs/promises";

const keys: { privateKey: string; publicKey: string }[] = [];
const NUMBER_OF_KEYS = 25;

describe("Generate keys", async () => {
  it("should generate keys", async () => {
    for (let i = 0; i < NUMBER_OF_KEYS; i++) {
      const privateKey = PrivateKey.random();
      const publicKey = privateKey.toPublicKey();
      keys.push({
        privateKey: privateKey.toBase58(),
        publicKey: publicKey.toBase58(),
      });
    }
    await writeFile("./data/keys.json", JSON.stringify({ keys }, null, 2));
  });
});
