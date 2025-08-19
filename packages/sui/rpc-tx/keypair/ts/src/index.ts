import { Ed25519Keypair } from "@mysten/sui/keypairs/ed25519";
import dotenv from "dotenv";

dotenv.config();

async function getSuiAddress(params: { secretKey: string }): Promise<string> {
  return Ed25519Keypair.fromSecretKey(params.secretKey)
    .getPublicKey()
    .toSuiAddress();
}

async function testEnvKeypair() {
  const suiSecretKey = process.env.SUI_SECRET_KEY;
  const expectedAddress = process.env.SUI_ADDRESS;

  if (!suiSecretKey) {
    throw new Error("SUI_SECRET_KEY not found in .env");
  }
  if (!expectedAddress) {
    throw new Error("SUI_ADDRESS not found in .env");
  }

  console.log("--- TypeScript Environment Test ---");
  console.log("Expected Address:", expectedAddress);

  try {
    const derivedAddress = await getSuiAddress({ secretKey: suiSecretKey });
    console.log("Derived Address: ", derivedAddress);
    console.log("Addresses match: ", derivedAddress === expectedAddress);

    if (derivedAddress === expectedAddress) {
      console.log("✓ TypeScript environment test passed!");
    } else {
      console.log("✗ TypeScript environment test failed!");
      process.exit(1);
    }
  } catch (error) {
    console.error("Error deriving address:", error);
    process.exit(1);
  }
}

testEnvKeypair().catch(console.error);