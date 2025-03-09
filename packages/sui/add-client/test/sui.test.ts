import { describe, it } from "node:test";
import assert from "node:assert";
import {
  CoinBalance,
  getFullnodeUrl,
  SuiClient,
  SuiEvent,
} from "@mysten/sui/client";
import { getFaucetHost, requestSuiFromFaucetV1 } from "@mysten/sui/faucet";
import { MIST_PER_SUI } from "@mysten/sui/utils";
import { Ed25519Keypair } from "@mysten/sui/keypairs/ed25519";
import { Secp256k1Keypair } from "@mysten/sui/keypairs/secp256k1";
import { Transaction, TransactionArgument } from "@mysten/sui/transactions";
import crypto from "node:crypto";
import secp256k1 from "secp256k1";

const suiClient = new SuiClient({
  url: getFullnodeUrl("localnet"),
});

describe("Sui test", async () => {
  it("should test sui txs", async () => {
    // // replace <YOUR_SUI_ADDRESS> with your actual address, which is in the form 0x123...
    // const MY_ADDRESS =
    //   "0xd61186512f3b2a4f5fa2d98bcab8f6a73a3806b1ed9611637f6944c7b4e079e7";
    // // random Keypair
    // const keypair = new Ed25519Keypair();
    // const publicKey = keypair.getPublicKey().toSuiAddress();
    // const secretKey = keypair.getSecretKey();

    const keypair = new Secp256k1Keypair();
    const address = keypair.getPublicKey().toSuiAddress();
    const secretKey = keypair.getSecretKey();

    console.log("secretKey", secretKey);
    console.log("address", address);
    return;
    // // Keypair from an existing secret key (Uint8Array)
    // const keypair2 = Ed25519Keypair.fromSecretKey(secretKey);
    // const address = keypair2.getPublicKey().toSuiAddress();
    // console.log("publicKey 2", address);

    // // create a new SuiClient object pointing to the network you want to use

    // const id = await suiClient.getChainIdentifier();
    // console.log("id", id);
    // const balance = await suiClient.getBalance({
    //   owner: MY_ADDRESS,
    //   coinType: "0x2::sui::SUI",
    // });
    // console.log("balance", balance);

    // Convert MIST to Sui
    const formatBalance = (balance: CoinBalance) => {
      return Number.parseInt(balance.totalBalance) / Number(MIST_PER_SUI);
    };

    // // store the JSON representation for the SUI the address owns before using faucet
    // try {
    //   const suiBefore = await suiClient.getBalance({
    //     owner: address,
    //   });
    //   console.log(`Balance before faucet: ${formatBalance(suiBefore)} SUI`);
    // } catch (error) {
    //   console.log("error getting balance", error);
    // }

    // await requestSuiFromFaucetV1({
    //   // use getFaucetHost to make sure you're using correct faucet address
    //   // you can also just use the address (see Sui TypeScript SDK Quick Start for values)
    //   host: getFaucetHost("localnet"),
    //   recipient: address,
    // });

    // // store the JSON representation for the SUI the address owns after using faucet
    // const suiAfter = await suiClient.getBalance({
    //   owner: address,
    //   coinType: "0x2::sui::SUI",
    // });

    // // Output result to console.
    // console.log(`Balance after faucet: ${formatBalance(suiAfter)} SUI.`);

    const packageID =
      "0xeaf0b5bed9f3110355b87dd6d9142365c75fa4a5f45cbfd68a894efa674fe46a";
    const object =
      "0x7e0327637f714cb647e21fc949aab11aa568ee37453a5c5284d08383915a35f7";
    // const keypair3 = Ed25519Keypair.fromSecretKey(
    //   "suiprivkey1qzkl4823wtjepelra5shdlc4jdecpqhhmp77vref5pw44wge38qt756cxed"
    // ); // 0x65b16ea406b74d8683e7819b1f12fec5f351f62369a7c6c744866388f952f1b9

    const keypair3 = Secp256k1Keypair.fromSecretKey(
      "suiprivkey1qxwa09t4hhyrskz5vstcgjdhudtw9sr8rh0qkzunng359jy9pe9lqy7f306"
    ); // 0xc3af09dbd444d54bc9ca64fb7f8cb1b95cb63c8d51eac3227096bb8f049166da
    const publicKey = keypair3.getPublicKey();
    console.log("publicKey", publicKey.toRawBytes());

    const data: Uint8Array = new Uint8Array([1, 2, 3]);
    const signedData = await keypair3.sign(data);

    const hash = crypto.createHash("sha256");
    hash.update(data);
    const messageHash = hash.digest();
    console.log(
      "messageHash",
      Array.from(messageHash)
        .map((b) => b.toString(16).padStart(2, "0"))
        .join("")
    );
    console.log(
      "signedData",
      Array.from(signedData)
        .map((b) => b.toString(16).padStart(2, "0"))
        .join("")
    );
    console.log(
      "publicKey",
      Array.from(publicKey.toRawBytes())
        .map((b) => b.toString(16).padStart(2, "0"))
        .join("")
    );

    const verified = secp256k1.ecdsaVerify(
      signedData,
      messageHash,
      publicKey.toRawBytes()
    );
    console.log("signature verified:", verified);

    console.log("signedData", signedData);

    console.time("tx build");
    const tx = new Transaction();

    // tx.moveCall({
    //   target: `${packageID}::add::create`,
    // });

    // tx.moveCall({
    //   target: `${packageID}::add::add`,
    //   arguments: [tx.object(object), tx.pure.u64(8)],
    // });

    tx.moveCall({
      target: `${packageID}::add::add_signed`,
      arguments: [
        tx.object(object),
        tx.pure.u64(8),
        tx.pure.vector("u8", signedData),
        tx.pure.vector("u8", publicKey.toRawBytes()),
      ],
    });
    console.timeEnd("tx build");

    console.time("tx sign");
    const result = await suiClient.signAndExecuteTransaction({
      signer: keypair3,
      transaction: tx,
    });
    console.timeEnd("tx sign");
    console.log("tx result", result);

    console.time("tx wait");
    const txWaitResult = await suiClient.waitForTransaction({
      digest: result.digest,
      options: {
        showEffects: true,
        showObjectChanges: true,
        showInput: true,
        showEvents: true,
        showBalanceChanges: true,
      },
    });
    console.timeEnd("tx wait");
    console.log("tx wait result", txWaitResult);
    console.log("events", (txWaitResult.events as SuiEvent[])?.[0]?.parsedJson);
  });
});
