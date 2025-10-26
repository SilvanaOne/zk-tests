import { describe, it } from "node:test";
import assert from "node:assert";
import { Field, Experimental, Gadgets } from "o1js";
import { writeFileSync } from "fs";
const { ZkFunction } = Experimental;

describe("Rust", () => {
  it("should create a proof for rust code", async () => {
    /**
     * Public input: a field element x
     *
     * Prove:
     *   I know a value y < 2^64 that is a cube root of x.
     */
    const main = ZkFunction({
      name: "Main",
      publicInputType: Field,
      privateInputTypes: [Field],
      main: (x: Field, y: Field) => {
        Gadgets.rangeCheck64(y);
        let y3 = y.square().mul(y);
        y3.assertEquals(x);
      },
    });

    /* console.time('compile...'); */
    const { verificationKey } = await main.compile();
    const x = Field(8);
    const y = Field(2);
    const proof = await main.prove(x, y);

    let ok = await main.verify(proof, verificationKey);

    console.log("ok?", ok);

    console.log("testing round trips");

    ok = await proofRoundTrip(proof).verify(verificationKey);
    console.log("proof round trip ok?", ok);

    console.log("verification key round trip...");
    // ok = await proof.verify(verificationKeyRoundTrip(verificationKey));

    console.time("proof verified");

    ok = await proof.verify(verificationKey);
    console.timeEnd("proof verified");

    console.log("verification key round trip ok?", ok);

    console.log("writing proof to file...");
    writeFileSync(
      "zkfunction-proof.json",
      JSON.stringify(proofRoundTrip(proof).toJSON(), null, 2)
    );

    console.log("writing verification key to file...");
    writeFileSync(
      "zkfunction-verification-key.data",
      verificationKeyRoundTrip(verificationKey).toString()
    );
  });
});

function proofRoundTrip(
  proof: Experimental.KimchiProof
): Experimental.KimchiProof {
  let json = proof.toJSON();
  console.log("proof json:", {
    proof: json.proof.slice(0, 10),
    publicInputFields: json.publicInputFields,
  });
  return Experimental.KimchiProof.fromJSON(json);
}

function verificationKeyRoundTrip(
  vk: Experimental.KimchiVerificationKey
): Experimental.KimchiVerificationKey {
  let json = vk.toString();
  console.log("vk string:", json.slice(0, 10));
  return Experimental.KimchiVerificationKey.fromString(json);
}
