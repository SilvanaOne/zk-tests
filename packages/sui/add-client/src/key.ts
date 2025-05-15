import {
  CoinBalance,
  getFullnodeUrl,
  SuiClient,
  SuiEvent,
} from "@mysten/sui/client";
import { getFaucetHost, requestSuiFromFaucetV2 } from "@mysten/sui/faucet";
import { MIST_PER_SUI } from "@mysten/sui/utils";
import { Ed25519Keypair } from "@mysten/sui/keypairs/ed25519";
import { Secp256k1Keypair } from "@mysten/sui/keypairs/secp256k1";

export function suiBalance(balance: CoinBalance): number {
  return Number.parseInt(balance.totalBalance) / Number(MIST_PER_SUI);
}

const MIN_SUI_BALANCE = 1;

export async function getKey(params: {
  network: "testnet" | "devnet" | "localnet";
  secretKey?: string;
}): Promise<{
  address: string;
  secretKey: string;
  keypair: Secp256k1Keypair;
  balance: CoinBalance;
}> {
  const { network } = params;
  const suiClient = new SuiClient({
    url: getFullnodeUrl(network),
  });
  let secretKey: string | undefined = params.secretKey;
  let address: string;
  let keypair: Secp256k1Keypair;
  if (!secretKey || secretKey === "0") {
    keypair = new Secp256k1Keypair();
    secretKey = keypair.getSecretKey();
  } else {
    keypair = Secp256k1Keypair.fromSecretKey(secretKey);
  }
  address = keypair.getPublicKey().toSuiAddress();
  let balance = await suiClient.getBalance({
    owner: address,
    coinType: "0x2::sui::SUI",
  });
  if (
    suiBalance(balance) < MIN_SUI_BALANCE &&
    (network === "localnet" || network === "devnet" || network === "testnet")
  ) {
    // console.log(
    //   `Requesting SUI from faucet, current balance: ${suiBalance(balance)} SUI`
    // );
    const tx = await requestSuiFromFaucetV2({
      host: getFaucetHost(network),
      recipient: address,
    });
    console.log("Faucet tx status:", tx.status);
    while (suiBalance(balance) < MIN_SUI_BALANCE) {
      await new Promise((resolve) => setTimeout(resolve, 1000));
      balance = await suiClient.getBalance({
        owner: address,
        coinType: "0x2::sui::SUI",
      });
    }
  }

  console.log("Address", address);
  console.log("SecretKey", secretKey);
  console.log(`Balance: ${suiBalance(balance)} SUI`);
  return { address, secretKey, keypair, balance };
}
