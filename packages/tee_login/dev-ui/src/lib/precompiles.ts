// "use client";

// import init, { add, recover_mnemonic } from "@/precompiles/precompiles";

// let wasmInitialized = false;
// let wasmInitPromise: Promise<any> | null = null;

// async function ensureWasmInitialized() {
//   if (wasmInitialized) {
//     return;
//   }

//   if (!wasmInitPromise) {
//     wasmInitPromise = init();
//   }

//   await wasmInitPromise;
//   wasmInitialized = true;
// }

// export async function rust_add(a: number, b: number): Promise<number> {
//   await ensureWasmInitialized();
//   return add(a, b);
// }

// export async function rust_recover_mnemonic(
//   data: Uint8Array[]
// ): Promise<string> {
//   await ensureWasmInitialized();
//   const result = recover_mnemonic(data);
//   return result;
// }
