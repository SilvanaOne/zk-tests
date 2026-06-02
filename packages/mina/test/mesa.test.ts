import { describe, it } from "node:test";
import assert from "node:assert";
import { writeFileSync, readFileSync, mkdirSync, existsSync } from "node:fs";
import path from "node:path";
import {
  SmartContract,
  method,
  AccountUpdate,
  UInt64,
  state,
  State,
  UInt32,
  Mina,
  fetchAccount,
  checkZkappTransaction,
  Permissions,
} from "o1js";
import type { DeployArgs } from "o1js";
import {
  fetchMinaAccount,
  accountBalanceMina,
  getTxStatusFast,
} from "@silvana-one/mina-utils";
import { sleep } from "../src/sleep.js";
import { formatTime } from "../src/time.js";

// mesa devnet endpoints (same as test/zkapp.devnet.test.ts)
const minaUrl = "https://plain-1-graphql.mesa-mut.minaprotocol.com/graphql";
const archiveUrl = "https://archive-node-api.mesa-mut.minaprotocol.com/";
const CHAIN = "mina:devnet" as const; // passed explicitly to getTxStatusFast

// How many contracts to deploy per run. Override with COUNT env (e.g. COUNT=2 for a smoke test).
// Re-running accumulates more contracts (public-key filenames never collide).
const COUNT = Number(process.env.COUNT ?? 100);
const DATA_DIR = path.resolve(process.cwd(), "data", "mesa");

const DEPLOY_FEE = 100_000_000; // 0.1 MINA
const POLL_MS = 10_000; // 10s inclusion poll (inclusion can take up to ~10 min)
const LOG_EVERY_MS = 60_000; // throttle "still waiting" logs to once a minute
const RESEND_MS = 10_000;
const MAX_RESEND = 20; // cap so a permanently-rejected tx fails fast instead of looping forever

// Mode switch: when UPGRADE_PRIVATE_KEY is set, the file runs the upgrade test instead of the deploys.
const UPGRADE_PRIVATE_KEY = process.env.UPGRADE_PRIVATE_KEY;
const UPGRADE = !!UPGRADE_PRIVATE_KEY;

const TEST_ACCOUNT_PRIVATE_KEY = process.env.TEST_ACCOUNT_1_PRIVATE_KEY;
const INCREMENT = UInt64.from(1);

const { TestPublicKey } = Mina;
type TestPublicKey = Mina.TestPublicKey;

/**
 * V1 contract — copied from test/zkapp.devnet.test.ts, with a deploy() override that
 * explicitly makes the verification key upgradeable BY SIGNATURE. The saved private key
 * is what authorizes that later (proof-less) upgrade.
 */
class BalanceContract extends SmartContract {
  @state(UInt64) record = State<UInt64>(UInt64.zero);

  async deploy(args: DeployArgs = {}) {
    await super.deploy(args); // sets vk + Permissions.default()
    this.account.permissions.set({
      ...Permissions.default(), // editState/send stay proof, etc.
      setVerificationKey: Permissions.VerificationKey.signature(), // VK upgradeable by signature
    });
  }

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

/**
 * V2 contract — the upgrade target. Adds a second state field `version` and a method to
 * set it. A different circuit ⇒ a different verification key, so the upgrade is observable
 * on-chain. NOTE: the deploy() override below is only used if V2 is deployed from scratch;
 * in the upgrade path V2 is only compiled (for its VK) and instantiated (to call setVersion).
 * The lock-down is applied directly in the upgrade transaction's account update.
 */
class BalanceContractV2 extends SmartContract {
  @state(UInt64) record = State<UInt64>(UInt64.zero);
  @state(UInt64) version = State<UInt64>(UInt64.zero);

  async deploy(args: DeployArgs = {}) {
    await super.deploy(args);
    this.account.permissions.set({
      ...Permissions.default(),
      setVerificationKey: Permissions.VerificationKey.signature(),
    });
  }

  @method
  public async setVersion(version: UInt64) {
    this.version.set(version);
  }
}

interface ContractRecord {
  index: number;
  publicKey: string;
  privateKey: string;
  vkHash: string | null; // original (deployed) verification key hash
  deployed: boolean;
  deployTxHash: string | null;
  txSentAt: string | null;
  deployedAt: string | null;
}

let sender: TestPublicKey;
let v1VkHash: string | null = null; // BalanceContract verification key hash (set at compile)
const keys: TestPublicKey[] = []; // index-aligned with records
const records: ContractRecord[] = []; // source of truth, flushed to disk
let flushed = false;
const start = Date.now();

// ---------------------------------------------------------------------------
// Key persistence helpers
// ---------------------------------------------------------------------------

function ensureDataDir(): void {
  if (!existsSync(DATA_DIR)) mkdirSync(DATA_DIR, { recursive: true });
}

function fileFor(publicKey: string): string {
  return path.join(DATA_DIR, `${publicKey}.json`);
}

function writeRecord(rec: ContractRecord): void {
  writeFileSync(fileFor(rec.publicKey), JSON.stringify(rec, null, 2));
}

/** Merge `patch` into the contract's JSON file (creating it from the keypair if missing).
 * Used by the upgrade flow to persist upgrade progress (status / pending tx / timestamps). */
function patchFile(
  publicKey: string,
  privateKey: string,
  patch: Record<string, unknown>,
): void {
  ensureDataDir();
  const file = fileFor(publicKey);
  let saved: Record<string, unknown> = {};
  if (existsSync(file)) {
    try {
      saved = JSON.parse(readFileSync(file, "utf8"));
    } catch {
      saved = {};
    }
  }
  saved.publicKey = publicKey;
  saved.privateKey = privateKey;
  Object.assign(saved, patch);
  writeFileSync(file, JSON.stringify(saved, null, 2));
  console.log("updated", file);
}

/** Generate ONE fresh keypair, save it to its own file IMMEDIATELY (before the deploy tx is
 * sent), and track it in memory. Generating just-in-time means a Ctrl-C leaves only the keys
 * we actually attempted to deploy — never a pile of undeployed key files. */
function createKey(index: number): { key: TestPublicKey; rec: ContractRecord } {
  const key = TestPublicKey.random();
  const rec: ContractRecord = {
    index,
    publicKey: key.toBase58(),
    privateKey: key.key.toBase58(),
    vkHash: v1VkHash,
    deployed: false,
    deployTxHash: null,
    txSentAt: null,
    deployedAt: null,
  };
  keys[index] = key;
  records[index] = rec;
  writeRecord(rec); // persist BEFORE sending the deploy tx
  return { key, rec };
}

/** Re-flush every in-memory record. Guarded so it never double-writes and never throws. */
function flushAll(): void {
  if (flushed) return;
  flushed = true;
  try {
    if (records.length === 0) return;
    ensureDataDir();
    for (const rec of records) writeRecord(rec);
  } catch (e) {
    console.error("flushAll error", e);
  }
}

// Register at module load so a Ctrl-C during generation / compile / mid-loop still flushes.
function installSignalHandlers(): void {
  const onExit =
    (label: string, code: number) =>
    (err?: unknown): void => {
      console.error(
        `\n[${label}] flushing ${records.length} records to disk…`,
        err ?? "",
      );
      flushAll();
      process.exit(code);
    };
  process.once("SIGINT", onExit("SIGINT", 130));
  process.once("SIGTERM", onExit("SIGTERM", 143));
  process.once("uncaughtException", onExit("uncaughtException", 1));
  process.once("unhandledRejection", onExit("unhandledRejection", 1));
}
// Each key is written to disk the moment it is generated (just before its deploy), so this
// handler is a belt-and-suspenders re-flush that also captures the latest deployed status.
installSignalHandlers();

// ---------------------------------------------------------------------------
// Network / sender helpers (shared by both modes)
// ---------------------------------------------------------------------------

function setupNetwork(): void {
  const networkInstance = Mina.Network({
    mina: minaUrl,
    archive: archiveUrl,
    networkId: "testnet",
  });
  Mina.setActiveInstance(networkInstance);
}

function setSender(): void {
  if (!TEST_ACCOUNT_PRIVATE_KEY) {
    throw new Error("TEST_ACCOUNT_1_PRIVATE_KEY environment variable is not set");
  }
  sender = TestPublicKey.fromBase58(TEST_ACCOUNT_PRIVATE_KEY);
}

/** Wait until a freshly deployed zkApp account exists on-chain (deploy creates the account). */
async function waitForInclusion(
  label: string,
  contractKey: TestPublicKey,
  hash: string,
  tStart: number,
): Promise<void> {
  let lastLog = tStart; // stay quiet for the first minute (a "pending" line was just printed)
  for (;;) {
    const res = await fetchAccount({ publicKey: contractKey });
    if (res.account) {
      console.log(`${label} included in ${formatTime(Date.now() - tStart)}`);
      return;
    }
    const now = Date.now();
    if (now - lastLog >= LOG_EVERY_MS) {
      lastLog = now;
      let statusOk = false;
      try {
        const st = await getTxStatusFast({ hash, chain: CHAIN });
        statusOk = st.success === true && st.result === true;
      } catch {
        statusOk = false; // archive node may not see the hash yet; non-fatal
      }
      console.log(
        `\x1b[33m${label} still pending: ${formatTime(now - tStart)} (statusFast=${statusOk})\x1b[0m`,
      );
    }
    await sleep(POLL_MS);
  }
}

/** Poll checkZkappTransaction until the tx is INCLUDED in a block, then return its outcome.
 * A zkApp tx can be admitted to the mempool ("pending"), included, and still FAIL to apply
 * (the fee is charged anyway) — so "pending" is NOT success. We wait for a blockHeight, then
 * read success/failureReason. This is how we verify whether the VK upgrade actually applied. */
async function waitForTxResolved(
  label: string,
  hash: string,
  tStart: number,
): Promise<{ success: boolean; failureReason: string[][][] | null; blockHeight: number }> {
  let lastLog = tStart;
  for (;;) {
    let st: Awaited<ReturnType<typeof checkZkappTransaction>> | undefined;
    try {
      st = await checkZkappTransaction(hash);
    } catch {
      st = undefined; // node may not see the tx yet; keep polling
    }
    if (st && st.blockHeight !== undefined) {
      return { success: st.success, failureReason: st.failureReason, blockHeight: st.blockHeight };
    }
    const now = Date.now();
    if (now - lastLog >= LOG_EVERY_MS) {
      lastLog = now;
      console.log(
        `\x1b[33m${label} waiting for tx to be included: ${formatTime(now - tStart)}\x1b[0m`,
      );
    }
    await sleep(POLL_MS);
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("mesa: deploy 100 BalanceContracts (and signature VK upgrade)", () => {
  // ---- Deploy mode (default; skipped when UPGRADE_PRIVATE_KEY is set) ----

  it("should set up network and compile", { skip: UPGRADE }, async () => {
    ensureDataDir();
    setupNetwork();
    setSender();
    const balance = await accountBalanceMina(sender);
    console.log("sender", sender.toBase58());
    console.log("sender balance", balance);
    const required = COUNT * 1.1; // ~1 MINA new-account fee + ~0.1 MINA tx fee each
    if (balance < required) {
      console.log(
        `\x1b[33mWARNING: sender balance ${balance} < required ~${required} MINA for ${COUNT} deploys\x1b[0m`,
      );
    }
    console.time("compile");
    const vk = (await BalanceContract.compile()).verificationKey;
    console.timeEnd("compile");
    v1VkHash = vk.hash.toString();
    console.log("BalanceContract vk hash", v1VkHash);
  });

  it("should deploy all contracts sequentially", { skip: UPGRADE }, async () => {
    for (let i = 0; i < COUNT; i++) {
      const tStart = Date.now();

      // Generate this contract's key and persist it JUST before deploying, so a Ctrl-C never
      // leaves undeployed key files for contracts we didn't get to.
      const { key: contractKey, rec } = createKey(i);
      console.log(`#${i} generated ${rec.publicKey}`);

      // Fresh nonce — strictly sequential (we wait for inclusion before the next), so the
      // local nonce is always current. Never pre-batch or nonces collide.
      await fetchMinaAccount({ publicKey: sender, force: true });

      const contract = new BalanceContract(contractKey);
      const tx = await Mina.transaction(
        { sender, fee: DEPLOY_FEE, memo: `mesa deploy #${i}` },
        async () => {
          AccountUpdate.fundNewAccount(sender, 1); // 1 MINA account-creation fee
          await contract.deploy({});
        },
      );
      tx.sign([sender.key, contractKey.key]); // signature-authorized deploy (no prove)

      // Send, resending until the node accepts it as "pending".
      let txSent = await tx.safeSend();
      let resend = 0;
      while (txSent.status !== "pending") {
        resend++;
        console.log(
          `\x1b[31m#${i} resend ${resend}:\x1b[0m`,
          txSent.hash,
          txSent.status,
          txSent.errors,
        );
        if (resend > MAX_RESEND) {
          throw new Error(
            `#${i} could not reach "pending" after ${MAX_RESEND} resends: ${String(
              txSent.errors ?? "",
            )}`,
          );
        }
        await sleep(RESEND_MS);
        await fetchMinaAccount({ publicKey: sender, force: true });
        tx.sign([sender.key, contractKey.key]);
        txSent = await tx.safeSend();
      }
      console.log(`#${i} pending`, txSent.hash);
      // Persist the pending tx hash immediately — so a Ctrl-C during the wait still records it.
      rec.deployTxHash = txSent.hash;
      rec.txSentAt = new Date().toISOString();
      writeRecord(rec);

      // Wait for the deploy to be INCLUDED before deploying the next contract.
      await waitForInclusion(`#${i}`, contractKey, txSent.hash, tStart);

      rec.deployed = true;
      rec.deployedAt = new Date().toISOString();
      writeRecord(rec);
      console.log(
        `#${i} INCLUDED in ${formatTime(Date.now() - tStart)} (${i + 1}/${COUNT}) ${rec.publicKey}`,
      );
    }
  });

  it("should report statistics", { skip: UPGRADE }, async () => {
    const deployedCount = records.filter((r) => r.deployed).length;
    const totalTime = Date.now() - start;
    console.log(`deployed ${deployedCount}/${COUNT} contracts`);
    console.log(`total time: ${formatTime(totalTime)}`);
    if (deployedCount > 0) {
      console.log(`average time: ${formatTime(totalTime / deployedCount)}`);
    }
    console.log("keys saved in", DATA_DIR);
    flushAll();
    assert.strictEqual(deployedCount, COUNT);
  });

  // ---- Upgrade mode (only when UPGRADE_PRIVATE_KEY is set) ----

  it(
    "should upgrade a contract's verification key by signature",
    { skip: !UPGRADE },
    async () => {
      setupNetwork();
      setSender();

      const contractKey = TestPublicKey.fromBase58(UPGRADE_PRIVATE_KEY!);
      const upgradePub = contractKey.toBase58();
      const upgradePriv = contractKey.key.toBase58();
      console.log("upgrading contract", upgradePub);

      console.time("compile v2");
      const newVk = (await BalanceContractV2.compile()).verificationKey;
      console.timeEnd("compile v2");
      const newVkHash = newVk.hash.toString();
      console.log("BalanceContractV2 vk hash", newVkHash);

      await fetchMinaAccount({ publicKey: contractKey, force: true });
      await fetchMinaAccount({ publicKey: sender, force: true });
      const oldVkHash = Mina.getAccount(contractKey).zkapp?.verificationKey?.hash.toString();
      console.log(`${upgradePub} current (old) vk hash`, oldVkHash);

      // If the contract is already at the new VK it was upgraded (and locked) on a previous run.
      // Don't fail — warn, then still attempt the upgrade to observe the lock rejecting it.
      const alreadyAtNewVk = oldVkHash === newVkHash;
      if (alreadyAtNewVk) {
        console.log(
          `\x1b[33m${upgradePub} WARNING: already at the new VK — re-attempting the upgrade to verify the lock rejects it\x1b[0m`,
        );
      }

      const tStart = Date.now();
      const tx = await Mina.transaction(
        { sender, fee: DEPLOY_FEE, memo: "vk upgrade" },
        async () => {
          const update = AccountUpdate.createSigned(contractKey); // signature auth on the zkApp account
          update.account.verificationKey.set(newVk); // set the new VK (no proof)
          update.account.permissions.set({
            // LOCK DOWN: no further VK upgrades, ever — and freeze permissions so it can't be reopened.
            ...Permissions.default(),
            setVerificationKey:
              Permissions.VerificationKey.impossibleDuringCurrentVersion(),
            setPermissions: Permissions.impossible(),
          });
        },
      );
      tx.sign([sender.key, contractKey.key]); // contract key authorizes the VK change + the lock

      // Send the upgrade (resend until the node admits it to the mempool as "pending").
      let txSent = await tx.safeSend();
      let resend = 0;
      while (txSent.status !== "pending") {
        resend++;
        console.log(
          `\x1b[31m${upgradePub} upgrade resend ${resend}:\x1b[0m`,
          txSent.hash,
          txSent.status,
          txSent.errors,
        );
        if (resend > MAX_RESEND) {
          throw new Error(
            `upgrade could not reach "pending" after ${MAX_RESEND} resends: ${String(
              txSent.errors ?? "",
            )}`,
          );
        }
        await sleep(RESEND_MS);
        await fetchMinaAccount({ publicKey: sender, force: true });
        tx.sign([sender.key, contractKey.key]);
        txSent = await tx.safeSend();
      }
      console.log(`${upgradePub} upgrade pending`, txSent.hash);
      // Persist the pending upgrade tx immediately — so a Ctrl-C during the wait still records it.
      patchFile(upgradePub, upgradePriv, {
        upgradeStatus: "pending",
        upgradeTxHash: txSent.hash,
        upgradeSentAt: new Date().toISOString(),
        upgradeToVkHash: newVkHash,
      });

      // "pending" only means the mempool admitted the tx. A zkApp tx can be included and still
      // FAIL to apply (e.g. setVerificationKey=impossible rejects the VK change). Wait until it
      // is included, then read whether it APPLIED or FAILED — that's the real lock test.
      const outcome = await waitForTxResolved(upgradePub, txSent.hash, tStart);
      const failureReason = outcome.failureReason ? JSON.stringify(outcome.failureReason) : null;
      console.log(
        `${upgradePub} tx resolved at block ${outcome.blockHeight} in ${formatTime(
          Date.now() - tStart,
        )}: success=${outcome.success}${failureReason ? ` failureReason=${failureReason}` : ""}`,
      );

      if (alreadyAtNewVk) {
        // The contract was already upgraded and locked — this VK upgrade MUST fail on-chain.
        // (Note: reading account.permissions.setVerificationKey can misreport as `signature`
        // because of its txnVersion field; the authoritative signal is the tx FAILING to apply.)
        assert.strictEqual(
          outcome.success,
          false,
          "locked contract: the VK upgrade should FAIL to apply on-chain, but it succeeded",
        );
        console.log(
          `\x1b[32m${upgradePub} lock confirmed: VK upgrade rejected on-chain (block ${outcome.blockHeight}, failureReason=${failureReason})\x1b[0m`,
        );
        patchFile(upgradePub, upgradePriv, {
          upgradeStatus: "locked",
          lockVerifiedTxHash: txSent.hash,
          lockFailureReason: outcome.failureReason,
          lockVerifiedAt: new Date().toISOString(),
        });
        return;
      }

      // First upgrade — it must APPLY, and the on-chain VK must become the new one.
      assert.strictEqual(outcome.success, true, `VK upgrade failed to apply on-chain: ${failureReason}`);
      let onchainHash: string | undefined;
      for (let a = 0; a < 12; a++) {
        await fetchAccount({ publicKey: contractKey });
        onchainHash = Mina.getAccount(contractKey).zkapp?.verificationKey?.hash.toString();
        if (onchainHash === newVkHash) break;
        await sleep(POLL_MS); // applied but account read may briefly lag — quiet retry
      }
      console.log(`${upgradePub} VK upgraded in ${formatTime(Date.now() - tStart)} → ${onchainHash}`);
      assert.strictEqual(onchainHash, newVkHash);

      // Bookkeeping: record the completed upgrade (time / status / vk) in the key file.
      patchFile(upgradePub, upgradePriv, {
        upgraded: true,
        upgradeStatus: "included",
        upgradeTxHash: txSent.hash,
        upgradeBlockHeight: outcome.blockHeight,
        upgradeVkHash: newVkHash,
        upgradedAt: new Date().toISOString(),
      });
    },
  );
});
