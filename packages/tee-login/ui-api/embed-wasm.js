import fs from 'fs';
import path from 'path';

// Read the WASM file
const wasmPath = path.join(process.cwd(), 'src/pkg/precompiles_bg.wasm');
const wasmBuffer = fs.readFileSync(wasmPath);

// Convert to base64
const base64 = wasmBuffer.toString('base64');

// Generate JavaScript module
const jsContent = `// Auto-generated - do not edit
// This file contains the embedded WASM binary as base64

export const wasmBase64 = "${base64}";

export function getWasmBytes() {
  // Convert base64 to Uint8Array
  const binaryString = atob(wasmBase64);
  const bytes = new Uint8Array(binaryString.length);
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }
  return bytes;
}
`;

// Write the JavaScript module
const outputPath = path.join(process.cwd(), 'src/embedded-wasm.js');
fs.writeFileSync(outputPath, jsContent);

console.log(`WASM embedded successfully: ${wasmBuffer.length} bytes -> ${base64.length} base64 chars`); 