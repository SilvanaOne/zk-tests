import { describe, it } from "node:test";
import assert from "node:assert";
import {
  generateKeyPair,
  encryptWithPublicKey,
  decryptWithPrivateKey,
} from "../src/encrypt.js";

const publicKey =
  "MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAmB+DYQ7I+K5wxHyyDfS62ftuepFp47bHMCyvbW6zRQ5FrS0ylPgzirfNqOn3o3L0Cw4ydCzOI2H+6PJI1h/XO0TGpwbYabHhJKfw7kQyAOBix/eMpg+JMu/rjcuIYzmBs5t97ydkC66+dCAIIFdmmqwTJK2rEs2rIiyCsQ16uxFm30ds8sqkq9Pcd3oCyW0ey4j+68pDqFcbgXmHKVk4Mc1N744b+Ebx1pgSNvxTCzylZf3eXYZhl39NfsanSbTGpN4Q9+vzVKOi2pXLgLDAzVmml66wbrWnutqEEpTrK3eZPcvbCnrGOVXUMpUQ1DM2aaIua/9CQhhV7QbPO0h8YQIDAQAB";

describe("Test encryption", async () => {
  it("should test", async () => {
    const { privateKey, publicKey } = generateKeyPair();
    // console.log("privateKey", privateKey);
    // console.log("publicKey", publicKey);
    const text = Array(1000).fill("Lorem ipsum dolor sit amet").join(" ");
    const encrypted = encryptWithPublicKey({
      text,
      publicKey,
    });
    //console.log("encrypted", encrypted);
    const decrypted = decryptWithPrivateKey({
      encryptedData: encrypted,
      privateKey,
    });
    //console.log("decrypted", decrypted);
    assert.strictEqual(text, decrypted);
  });
});
