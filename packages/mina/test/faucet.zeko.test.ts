import { describe, it } from "node:test";
import assert from "node:assert";
import { PrivateKey, PublicKey, Mina, AccountUpdate, UInt64 } from "o1js";
//import { config } from "dotenv";
import {
  initBlockchain,
  accountBalanceMina,
  accountBalance,
  Zeko,
  Devnet,
  fetchMinaAccount,
} from "@silvana-one/mina-utils";
import { sleep } from "../src/sleep.js";
import { faucet } from "../src/faucet.js";

// Load environment variables from .env.zeko
//config({ path: ".env.zeko" });

const chain = process.env.CHAIN;
const MINIMUM_AMOUNT = 1000;
const keysToTopup: string[] = [];

// Load keys from environment variables
function loadKeysFromEnv(): { privateKey: string; publicKey: string }[] {
  console.log("Loading keys from .env.zeko");
  const keys: { privateKey: string; publicKey: string }[] = [];
  let i = 1;
  while (
    i < 25 &&
    process.env[`TEST_ACCOUNT_${i}_PRIVATE_KEY`] &&
    process.env[`TEST_ACCOUNT_${i}_PUBLIC_KEY`]
  ) {
    keys.push({
      privateKey: process.env[`TEST_ACCOUNT_${i}_PRIVATE_KEY`]!,
      publicKey: process.env[`TEST_ACCOUNT_${i}_PUBLIC_KEY`]!,
    });
    i++;
    console.log(`Loaded ${i} key from .env.zeko`);
  }
  return keys;
}

describe("Topup Zeko", async () => {
  it("should init blockchain", async () => {
    console.log({ chain });
    if (chain !== "zeko") {
      throw new Error("Invalid chain");
    }
    await initBlockchain(chain);
  });
  it.skip("should check balances on zeko network", async () => {
    // Load keys from environment variables
    const keys = loadKeysFromEnv();
    console.log(`Loaded ${keys.length} accounts from .env.zeko`);

    console.log("\n=== Account Balances on Zeko Network ===");
    for (let i = 0; i < keys.length; i++) {
      const { privateKey, publicKey } = keys[i];

      // Verify key pair consistency
      assert.strictEqual(
        PrivateKey.fromBase58(privateKey).toPublicKey().toBase58(),
        publicKey
      );

      const balance = await accountBalanceMina(PublicKey.fromBase58(publicKey));
      console.log(`Account ${i + 1}: ${publicKey} -> ${balance} MINA`);

      if (balance < MINIMUM_AMOUNT) {
        keysToTopup.push(publicKey);
      }
    }

    console.log(`\nAccounts needing topup: ${keysToTopup.length}`);
    if (keysToTopup.length > 0) {
      console.log("Addresses to topup:");
      keysToTopup.forEach((addr, idx) => {
        console.log(`  ${idx + 1}. ${addr}`);
      });
    }
  });
  it.skip("should topup directly accounts on zeko", async () => {
    for (const publicKey of keysToTopup) {
      const response = await faucet({
        publicKey,
        explorerUrl: Zeko.explorerAccountUrl ?? "",
        network: "devnet",
        faucetUrl: "https://zeko.io/api/faucet",
      });
      if (response.success !== true) {
        console.log("faucet error:", response);
        await sleep(10_000);
      }
      await sleep(5_000);
      const balance = await accountBalanceMina(PublicKey.fromBase58(publicKey));
      console.log(`${publicKey}: ${balance} MINA`);
    }
  });
  let i = 0;
  while (i < 10) {
    i++;
    it("should topup accounts using temp keys", async () => {
      const tanks: {
        privateKey: string;
        publicKey: string;
        targetAddress: string;
      }[] = [];

      for (let i = 0; i < keysToTopup.length; i++) {
        const targetAddress = keysToTopup[i];
        try {
          const privateKey = PrivateKey.random();
          const publicKey = privateKey.toPublicKey();

          console.log(
            `Requesting faucet for temp key ${i + 1}: ${publicKey.toBase58()}`
          );
          const response = await faucet({
            publicKey: publicKey.toBase58(),
            explorerUrl: Zeko.explorerAccountUrl ?? "",
            network: "devnet",
            faucetUrl: "https://zeko.io/api/faucet",
          });
          if (response.success !== true) {
            console.log("faucet error:", response);
            await sleep(10_000);
          } else {
            tanks.push({
              privateKey: privateKey.toBase58(),
              publicKey: publicKey.toBase58(),
              targetAddress,
            });
          }
        } catch (e) {
          console.log("Faucet error:", e);
          await sleep(60000);
        }
        await sleep(10000);
      }

      for (let i = 0; i < tanks.length; i++) {
        try {
          const tank = tanks[i];
          const tempPublicKey = PublicKey.fromBase58(tank.publicKey);
          const targetPublicKey = PublicKey.fromBase58(tank.targetAddress);

          // Wait for balance to be available
          let balance = await accountBalance(tempPublicKey);
          console.log(
            `Temp account ${i + 1}: ${tempPublicKey.toBase58()}: ${
              balance.toBigInt() / 1_000_000_000n
            } MINA`
          );

          while (balance.toBigInt() < 5_000_000_000n) {
            await sleep(10000);
            balance = await accountBalance(tempPublicKey);
          }

          const sender = tempPublicKey;
          const receiver = targetPublicKey;
          const deployer = PrivateKey.fromBase58(tank.privateKey);
          const fee = UInt64.from(500_000_000);

          await fetchMinaAccount({
            publicKey: receiver,
            force: false,
          });

          const isNew = !Mina.hasAccount(receiver);
          const amount = balance.sub(
            fee.add(UInt64.from(isNew ? 100_000_000 : 0))
          );

          const transaction = await Mina.transaction(
            { sender, fee },
            async () => {
              if (isNew) AccountUpdate.fundNewAccount(sender, 1);
              const senderUpdate = AccountUpdate.createSigned(sender);
              senderUpdate.send({
                to: receiver,
                amount,
              });
            }
          );

          const txSent = await transaction.sign([deployer]).send();
          console.log(
            `✓ Transferred ${
              amount.toBigInt() / 1_000_000_000n
            } MINA to ${receiver.toBase58()}`
          );
          console.log(`  Transaction hash: ${txSent.hash}`);

          await sleep(5000);
        } catch (e) {
          console.log(`Error transferring for account ${i + 1}:`, e);
          await sleep(60000);
        }
      }
    });
  }
});
