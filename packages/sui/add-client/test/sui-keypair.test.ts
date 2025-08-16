import { describe, it } from "node:test";
import assert from "node:assert";
import { Ed25519Keypair } from "@mysten/sui/keypairs/ed25519";

describe("Generate Sui keypair", async () => {
  it("should generate sui keypair", async () => {
    const keypair = new Ed25519Keypair();
    const address = keypair.getPublicKey().toSuiAddress();
    const secretKey = keypair.getSecretKey();

    console.log("address", address);
    console.log("secret key", secretKey);
  });
});
