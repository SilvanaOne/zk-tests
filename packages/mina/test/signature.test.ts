import { describe, it } from "node:test";
import assert from "node:assert";
import { verifyFields } from "../src/signature/mina-signer.js";
import { verifySignature } from "../src/verify/signature.js";
import { poseidon } from "../src/verify/hash.js";

describe("Signature", () => {
  it(`should verify signature with strings`, async () => {
    const verified = verifyFields({
      data: [123n, 456n],
      signature:
        "7mXSdHUousVBHBvnrqwjhGPkMWcokkDzv5P4AH93eUR7xegDrtms8uZeoQN98A6c3uA68M3hCD2TqRx1NKXfLTbdZ2tH6Sdo",
      publicKey: "B62qqdx1tLZix6NFPzytZZwppGiXBPrsxByjyDMs9yp9XmuVmYHDXqN",
    });
    console.log("verified:", verified);
    assert.strictEqual(verified, true);
  });

  it(`should verify signature with bigint`, async () => {
    const signature = {
      r: 19996910013141570341263734673999978016031842709489071252992906391155381778902n,
      s: 3955505917773286787189402766131368989164806632077003535544705187409914383142n,
    };
    const publicKey = {
      x: 23870790172301888504759036806304867767472357997524493282691794869180801897430n,
      isOdd: true,
    };
    const verified = verifySignature({
      data: [123n, 456n],
      signature,
      publicKey,
    });
    console.log("verified 2:", verified);
    assert.strictEqual(verified, true);
  });

  it(`should verify signature and hash`, async () => {
    const message3 = [659432634n, 129582692366n];
    const hash3 =
      14511186988113284420166449852209351846476681225734727260090304858685254164294n;
    const publicKey3 = {
      x: 26972722282908082823709548943628812107630160480671449039226625034450942162759n,
      isOdd: true,
    };

    const signature3 = {
      r: 5401442348387579931456672613506717321719092304896109961240285669716771848845n,
      s: 22475384102374837400438140879721219833132447527208379393766446061882354260257n,
    };
    const verified = verifySignature({
      data: message3,
      signature: signature3,
      publicKey: publicKey3,
    });
    console.log("verified 3:", verified);
    assert.strictEqual(verified, true);
    const hash = poseidon(message3);
    console.log("hash:", hash);
    assert.strictEqual(hash, hash3);
  });
});
