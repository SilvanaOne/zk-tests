import { describe, it } from "node:test";
import assert from "node:assert";
import {
  CoinBalance,
  getFullnodeUrl,
  SuiClient,
  SuiEvent,
} from "@mysten/sui/client";
import { getFaucetHost, requestSuiFromFaucetV2 } from "@mysten/sui/faucet";
import { MIST_PER_SUI } from "@mysten/sui/utils";
import { Ed25519Keypair, Ed25519PublicKey } from "@mysten/sui/keypairs/ed25519";
import { Secp256k1Keypair } from "@mysten/sui/keypairs/secp256k1";
import { Transaction, TransactionArgument } from "@mysten/sui/transactions";
import crypto from "node:crypto";
import secp256k1 from "secp256k1";
import { MultiSigPublicKey } from "@mysten/sui/multisig";

const suiClient = new SuiClient({
  url: getFullnodeUrl("devnet"),
});

describe("Multisig test", async () => {
  it("should test single signature", async () => {
    const keypair = new Ed25519Keypair();
    const publicKey = keypair.getPublicKey();
    const publicKeyBase64 = publicKey.toBase64();
    console.log("publicKeyBase64", publicKeyBase64);
    const message = new TextEncoder().encode("hello world");

    const { signature } = await keypair.signPersonalMessage(message);
    const recoveredPublicKey = new Ed25519PublicKey(publicKeyBase64);
    console.log("recoveredPublicKey", recoveredPublicKey.toBase64());
    const isValid = await recoveredPublicKey.verifyPersonalMessage(
      message,
      signature
    );
    console.log("isValid single signature", isValid);
  });
  it("should test multisig", async () => {
    const kp1 = new Ed25519Keypair();
    const kp2 = new Ed25519Keypair();
    const kp3 = new Ed25519Keypair();

    const multiSigPublicKey = MultiSigPublicKey.fromPublicKeys({
      threshold: 2,
      publicKeys: [
        {
          publicKey: kp1.getPublicKey(),
          weight: 1,
        },
        {
          publicKey: kp2.getPublicKey(),
          weight: 1,
        },
        {
          publicKey: kp3.getPublicKey(),
          weight: 2,
        },
      ],
    });
    const publicKey = multiSigPublicKey.toBase64();
    console.log("publicKey", publicKey);
    const recoveredPublicKey = new MultiSigPublicKey(publicKey);
    console.log("recoveredPublicKey", recoveredPublicKey.toBase64());

    const multisigAddress = multiSigPublicKey.toSuiAddress();
    console.log("multisigAddress", multisigAddress);

    // This example uses the same imports, key pairs, and multiSigPublicKey from the previous example
    const message = new TextEncoder().encode("hello world");

    const signature1 = (await kp1.signPersonalMessage(message)).signature;
    const signature2 = (await kp2.signPersonalMessage(message)).signature;

    const combinedSignature = multiSigPublicKey.combinePartialSignatures([
      signature1,
      signature2,
    ]);

    const isValid = await multiSigPublicKey.verifyPersonalMessage(
      message,
      combinedSignature
    );

    console.log("isValid", isValid);
  });
});
