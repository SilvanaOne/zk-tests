import { describe, it } from "node:test";
import assert from "node:assert";
import {
  SmartContract,
  method,
  AccountUpdate,
  UInt64,
  state,
  State,
  Provable,
  Mina,
  fetchAccount,
  fetchLastBlock,
  UInt32,
  PublicKey,
} from "o1js";
import { fetchMinaAccount, accountBalanceMina } from "@silvana-one/mina-utils";
import { sleep } from "../src/sleep.js";
import { formatTime } from "../src/time.js";
import { fetchZekoFee } from "../src/zeko-fee.js";

//const url = "http://m1.zeko.io/graphql";
//const url = "https://devnet.zeko.io/graphql";
const minaUrl = "https://plain-1-graphql.mesa-mut.minaprotocol.com/graphql";
const archiveUrl = "https://archive-node-api.mesa-mut.minaprotocol.com/";
// const minaUrl = "https://api.minascan.io/node/devnet/v1/graphql";
// const archiveUrl = "https://api.minascan.io/archive/devnet/v1/graphql";
const COUNT = 2;

const { TestPublicKey } = Mina;
type TestPublicKey = Mina.TestPublicKey;
const zkKey = TestPublicKey.random();
const zkApp = //zkKey;
  PublicKey.fromBase58(
    "B62qkspCMH2cspD9PpwtktjnPj4sF9yzNKkVTF6UNgBWxCv2PLLeo43",
  );

const TEST_ACCOUNT_PRIVATE_KEY = process.env.TEST_ACCOUNT_1_PRIVATE_KEY;
const INCREMENT = UInt64.from(1);
let sender: TestPublicKey;
let applied = 0;
let start = Date.now();

class BalanceContract extends SmartContract {
  @state(UInt64) record = State<UInt64>(UInt64.zero);

  @method
  public async topup(expiry: UInt32) {
    this.network.globalSlotSinceGenesis.requireBetween(UInt32.zero, expiry);
    const balance = this.account.balance.getAndRequireEquals();
    const record = this.record.getAndRequireEquals();
    balance.assertEquals(record.mul(4));
    const sender = this.sender.getUnconstrained();
    const senderUpdate = AccountUpdate.createSigned(sender);
    const senderUpdate2 = AccountUpdate.createSigned(sender);
    const senderUpdate3 = AccountUpdate.createSigned(sender);
    const senderUpdate4 = AccountUpdate.createSigned(sender);
    senderUpdate.balance.subInPlace(INCREMENT);
    senderUpdate2.balance.subInPlace(INCREMENT);
    senderUpdate3.balance.subInPlace(INCREMENT);
    senderUpdate4.balance.subInPlace(INCREMENT);
    this.balance.addInPlace(INCREMENT.mul(4));
    this.record.set(record.add(INCREMENT));
  }
}

const balanceContract = new BalanceContract(zkApp);

describe("balance instability check", () => {
  it(`should compile`, async () => {
    const networkInstance = Mina.Network({
      mina: minaUrl,
      archive: archiveUrl,
      networkId: "testnet",
    });
    Mina.setActiveInstance(networkInstance);
    if (!TEST_ACCOUNT_PRIVATE_KEY) {
      throw new Error(
        "TEST_ACCOUNT_PRIVATE_KEY environment variable is not set",
      );
    }
    sender = TestPublicKey.fromBase58(TEST_ACCOUNT_PRIVATE_KEY);
    const balance = await accountBalanceMina(sender);
    console.log("balance", balance);

    const vk = (await BalanceContract.compile()).verificationKey;
    console.log("vk", vk.hash.toJSON());
    console.log("max slot", UInt32.MAXINT().toBigint());
  });
  it.skip(`should deploy`, async () => {
    await fetchMinaAccount({ publicKey: sender, force: true });
    console.log("sender", sender.toJSON());
    console.log("sender balance", await accountBalanceMina(sender));
    console.log("contract", zkKey.toBase58());
    console.log("zkApp private key", zkKey.key.toBase58());
    // const lastBlock = await fetchLastBlock();
    // console.log("last slot", lastBlock.globalSlotSinceGenesis);
    const tx = await Mina.transaction(
      { sender, fee: 100_000_000, memo: "balance contract deploy" },
      async () => {
        AccountUpdate.fundNewAccount(sender, 1);
        //const au = AccountUpdate.createSigned(sender);
        //au.balance.subInPlace(UInt64.from(100_000_000));
        await balanceContract.deploy({});
      },
    );

    tx.sign([sender.key, zkKey.key]);

    let txSent = await tx.safeSend();
    console.timeEnd("send");
    console.log("deploy txSent", txSent.hash, txSent.status);
    console.log(`deploy time: ${formatTime(Date.now() - start)}`);
    start = Date.now();
  });

  for (let i = 0; i < COUNT; i++) {
    it(`should run ${i}`, async () => {
      const timeStart = Date.now();
      await fetchMinaAccount({ publicKey: sender, force: true });
      await fetchMinaAccount({ publicKey: zkApp, force: true });
      let balance = Mina.getAccount(zkApp).balance.toBigInt();
      let record = balanceContract.record.get().toBigInt();
      let attempt = 1;
      console.log(`balance ${i}:`, Number(balance));
      console.log(`record ${i}:`, Number(record));

      while (Number(balance) !== i * 4 || Number(record) !== i) {
        console.log(
          `\x1b[31mwaiting for balance ${i}: ${formatTime(
            Date.now() - timeStart,
          )}\x1b[0m`,
        );
        await sleep(10000 * attempt);
        attempt++;
        await fetchMinaAccount({ publicKey: sender, force: true });
        await fetchMinaAccount({ publicKey: zkApp, force: true });
        balance = Mina.getAccount(zkApp).balance.toBigInt();
        record = balanceContract.record.get().toBigInt();
        console.log(`balance retry ${i}:`, Number(balance));
        console.log(`record retry ${i}:`, Number(record));
      }
      console.log(`balance ${i}:`, Number(balance));
      console.log(`record ${i}:`, Number(record));
      assert.strictEqual(balance, record * 4n);
      assert.strictEqual(Number(balance), i * 4);
      console.log(`iteration ${i} completed`);

      const tx = await Mina.transaction(
        { sender, fee: 100_000_000, memo: `step ${i + 1}` },
        async () => {
          await balanceContract.topup(UInt32.from(1_000_000n));
        },
      );
      await tx.prove();

      tx.sign([sender.key]);
      console.time("sendTx");
      let txSent = await tx.safeSend();

      console.timeEnd("sendTx");
      console.log(`txSent ${i}:`, txSent.hash, txSent.status);
      console.time("applied");
      const startApplied = Date.now();
      await fetchAccount({ publicKey: zkApp });
      let recordSent = balanceContract.record.get().toBigInt();
      let attemptSent = 1;

      while (Number(recordSent) !== i + 1) {
        attemptSent++;
        await fetchAccount({ publicKey: zkApp });
        recordSent = balanceContract.record.get().toBigInt();
      }
      console.timeEnd("applied");
      const appliedTime = Date.now() - startApplied;
      console.log(`record ${i}:`, Number(recordSent));
      console.log(`attempt ${i}:`, attemptSent);
      console.log(`txSent ${i}:`, txSent.hash, txSent.status);
      if (appliedTime > applied) {
        applied = appliedTime;
      }
    });
  }
  it(`should calculate statistics`, async () => {
    const totalTime = Date.now() - start;
    console.log(`total time: ${formatTime(totalTime)}`);
    console.log(`applied:`, applied);
    console.log(`average time: ${formatTime(totalTime / COUNT)}`);
    console.log(`applied:`, applied);
  });
});
