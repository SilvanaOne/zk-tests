import { recover_mnemonic } from "./pkg/precompiles.js";
import { importWalletByMnemonic } from "./mina.js";

const { subtle } = crypto;

export async function generateKeyPair(): Promise<{
  publicKey: string;
  privateKey: CryptoKey | null;
}> {
  let { publicKey, privateKey } = await subtle.generateKey(
    {
      name: "RSA-OAEP",
      modulusLength: 4096,
      publicExponent: new Uint8Array([1, 0, 1]),
      hash: "SHA-512",
    },
    false, // extractable
    ["decrypt"] // usages
  );
  const spki = new Uint8Array(await subtle.exportKey("spki", publicKey)); //  [oai_citation:1‡developer.mozilla.org](https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/exportKey?utm_source=chatgpt.com)
  const pubB64 = btoa(String.fromCharCode(...spki));

  return { publicKey: pubB64, privateKey };
}

export async function decryptShares(params: {
  data: string[];
  privateKey: CryptoKey;
}): Promise<{
  privateKey: string;
  publicKey: string;
} | null> {
  console.log("decryptShares", params.data);
  const shares: Uint8Array[] = [];
  for (const share of params.data) {
    const shareDecrypted = await decrypt({
      encrypted: share,
      privateKey: params.privateKey,
    });
    if (shareDecrypted === null) {
      return null;
    }
    shares.push(shareDecrypted);
  }

  return await importWalletByMnemonic(recover_mnemonic(shares));
}

async function decrypt(params: {
  encrypted: string;
  privateKey: CryptoKey | null;
}): Promise<Uint8Array | null> {
  if (params.privateKey === null) {
    return null;
  }
  const cipher = Uint8Array.from(atob(params.encrypted), (c) =>
    c.charCodeAt(0)
  );
  const plainBuf = await subtle.decrypt(
    { name: "RSA-OAEP" },
    params.privateKey,
    cipher
  );
  params.privateKey = null;
  return new Uint8Array(plainBuf);
}
