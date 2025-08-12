import { describe, it } from "node:test";
import assert from "node:assert";
import { Poseidon, Signature, PublicKey, PrivateKey, Field } from "o1js";
import bs58 from "bs58";
import { initBlockchain } from "@silvana-one/mina-utils";

describe("Signature", () => {
  it(`should calculate hash`, async () => {
    const hash = Poseidon.hash(
      [240717916736854602989207148466022993262069182275n, 1n, 2n].map(Field)
    ).toJSON();
    console.log("hash:", hash);
  });

  it(`should sign and verify`, async () => {
    await initBlockchain("devnet");
    const privateKey = PrivateKey.fromBase58(
      "EKFXH5yESt7nsD1TJy5WNb4agVczkvzPRVexKQ8qYdNqauQRA8Ef"
    );
    const publicKey = privateKey.toPublicKey();
    console.log("publicKey:", publicKey.toBase58());
    const signature = Signature.create(privateKey, [1n, 2n, 3n].map(Field));
    console.log("signature:", signature.toJSON());
    const signature_base58 = signature.toBase58();
    console.log("signature:", signature_base58);
    const signature_hex = Buffer.from(bs58.decode(signature_base58)).toString(
      "hex"
    );
    console.log("signature_hex:", signature_hex);
    const verified = signature.verify(publicKey, [1n, 2n].map(Field));
    console.log("verified:", verified.toBoolean());
  });
});
