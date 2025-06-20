import * as bip32 from "bip32";
import * as bip39 from "bip39";
import bs58check from "bs58check";
import { Buffer } from "safe-buffer";
import Client from "mina-signer";

const client = new Client({
  network: "testnet",
});

export interface PaymentParams {
  from: string;
  to: string;
  amount: string;
  fee: string;
  nonce: number;
  memo: string;
}

export async function signPayment(params: {
  payment: string;
  privateKey: string;
}): Promise<string | undefined> {
  try {
    const { privateKey } = params;
    const payment: PaymentParams = JSON.parse(params.payment);
    return JSON.stringify(client.signPayment(payment, privateKey));
  } catch (error: any) {
    console.error("Error signing payment", error?.message);
    return undefined;
  }
}

export function signMessage(message: bigint[], privateKey: string): string {
  const signed = client.signFields(message, privateKey);
  return signed.signature;
}

export function getHDpath(account = 0) {
  const purpose = 44;
  const index = 0;
  const charge = 0;
  const hdPath =
    "m/" +
    purpose +
    "'/" +
    12586 +
    "'/" +
    account +
    "'/" +
    charge +
    "/" +
    index;
  return hdPath;
}

function reverse(bytes: any) {
  const reversed = new Buffer(bytes.length);
  for (let i = bytes.length; i > 0; i--) {
    (reversed as any)[bytes.length - i] = bytes[i - 1];
  }
  return reversed;
}

export async function importWalletByMnemonic(
  mnemonic: string,
  index = 0
): Promise<{
  privateKey: string;
  publicKey: string;
  hdIndex: number;
}> {
  const seed = bip39.mnemonicToSeedSync(mnemonic);
  const masterNode = bip32.fromSeed(seed);
  let hdPath = getHDpath(index);
  const child0 = masterNode.derivePath(hdPath);
  (child0.privateKey as any)[0] &= 0x3f;
  const childPrivateKey = reverse(child0.privateKey);
  const privateKeyHex = `5a01${childPrivateKey.toString("hex")}`;
  const privateKey = bs58check.encode(Buffer.from(privateKeyHex, "hex") as any);
  const publicKey = client.derivePublicKey(privateKey);
  return {
    privateKey,
    publicKey,
    hdIndex: index,
  };
}
