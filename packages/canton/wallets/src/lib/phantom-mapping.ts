"use client";

// Mapping of Solana public keys to Canton party IDs
const SOLANA_TO_PARTY_MAP: Record<string, string> = {
  DGT9WLSneH4TXNmdLv1AGmmUr31BZN8b81PRfjPfbfnR:
    "ext-user-phantom-1::12208bf4c7bd06398a912a294ecf703f22b6ba1f10d83088134b49b1539505aa21df",
  FNXv9G942d5JuYxKmX2tX4vWJhfzjDtcBFyCw2oRenys:
    "ext-user-phantom-1::12208bf4c7bd06398a912a294ecf703f22b6ba1f10d83088134b49b1539505aa21df",
  "5FRBdGM4prxzAvYqNcTfUnNyfpn2LUWL1psJNh1m9h4L":
    "ext-user-solflare-2::1220ceb5d4e19edb8118b1039c8587a3cc08afa5e496cb12fc4480181127cf0a76cd",
};

export function getPartyIdFromSolanaKey(publicKey: string): string | undefined {
  return SOLANA_TO_PARTY_MAP[publicKey];
}

export function getSolanaKeyFromPartyId(partyId: string): string | undefined {
  const entry = Object.entries(SOLANA_TO_PARTY_MAP).find(
    ([_, pid]) => pid === partyId
  );
  return entry?.[0];
}
