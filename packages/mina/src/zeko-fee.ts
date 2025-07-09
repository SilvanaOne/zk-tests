import { Transaction, UInt64 } from "o1js";

/// txn.setFee(await fetchZekoFee(txn));

export async function fetchZekoFee(params: {
  txn: Transaction<false, false> | number;
  buffer?: number;
  url?: string;
}): Promise<UInt64 | undefined> {
  const { txn, buffer = 0.1, url = "https://devnet.zeko.io/graphql" } = params;
  const weight =
    typeof txn === "number" ? txn : txn.transaction.accountUpdates.length + 1;
  //console.log("fetchZekoFee weight", weight);
  try {
    const response = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        query: `
        query FeePerWeight($weight: Int!) {
          feePerWeightUnit(weight: $weight)
        }
      `,
        variables: { weight },
      }),
    });

    const { data } = await response.json();
    if (!data || !data.feePerWeightUnit) {
      console.error("fetchZekoFee: Invalid response from Zeko", data);
      return undefined;
    }
    return UInt64.from(Math.ceil(data.feePerWeightUnit)).add(
      UInt64.from(buffer * 10e8)
    );
  } catch (error: any) {
    console.error("fetchZekoFee", error?.message ?? error);
    return undefined;
  }
}
