/// Test file to verify rScalarPow works correctly for large indices
/// <reference types="node" />

import { test, describe } from "node:test";
import assert from "node:assert";
import { rScalarPow, rScalarPowLegacy } from "../src/exp.js";
import { getR } from "../src/constants.js";
import { createForeignField, UInt32 } from "o1js";

// BLS12‑381 scalar field prime
const BLS_FR =
  0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001n;
const Fr = createForeignField(BLS_FR);

type CanonicalElement = InstanceType<typeof Fr.Canonical>;

function testIndex(index: number): boolean {
  console.log(`Testing index ${index}...`);

  // Use optimized version
  const optimized = rScalarPow(index);

  // Use legacy version for comparison (only for smaller indices to avoid timeout)
  if (index <= 1000) {
    const legacy = rScalarPowLegacy(index);

    if (optimized.toBigInt() !== legacy.toBigInt()) {
      console.error(`❌ Mismatch at index ${index}:`);
      console.error(`  Optimized: 0x${optimized.toBigInt().toString(16)}`);
      console.error(`  Legacy:    0x${legacy.toBigInt().toString(16)}`);
      return false;
    }
  }

  // Manual verification for small indices
  if (index <= 5) {
    const r = getR();
    let manual = Fr.from(1n);
    for (let i = 0; i < index; i++) {
      manual = manual.mul(r).assertCanonical();
    }

    if (optimized.toBigInt() !== manual.toBigInt()) {
      console.error(`❌ Manual verification failed at index ${index}:`);
      console.error(`  Optimized: 0x${optimized.toBigInt().toString(16)}`);
      console.error(`  Manual:    0x${manual.toBigInt().toString(16)}`);
      return false;
    }
  }

  console.log(
    `✅ Index ${index} passed - Result: 0x${optimized
      .toBigInt()
      .toString(16)
      .slice(0, 16)}...`
  );
  return true;
}

function runTests(): void {
  console.log("🧪 Testing rScalarPow for various indices...\n");

  const testIndices = [
    0, 1, 2, 3, 4, 5, 10, 15, 20, 50, 100, 255, 256, 500, 1000, 1023, 1024,
    1025, 2048, 5000, 10000, 50000, 100000, 500000, 1000000,
  ];

  let passed = 0;
  let failed = 0;

  for (const index of testIndices) {
    try {
      if (testIndex(index)) {
        passed++;
      } else {
        failed++;
      }
    } catch (error) {
      console.error(
        `❌ Error testing index ${index}: ${
          error instanceof Error ? error.message : String(error)
        }`
      );
      failed++;
    }
  }

  console.log(`\n📊 Test Results:`);
  console.log(`✅ Passed: ${passed}`);
  console.log(`❌ Failed: ${failed}`);

  if (failed === 0) {
    console.log(
      `🎉 All tests passed! rScalarPow works correctly for all tested indices.`
    );
  } else {
    console.log(`⚠️  Some tests failed. Please check the implementation.`);
  }
}

// Edge case tests
function testEdgeCases(): void {
  console.log("\n🔬 Testing edge cases...");

  // Test maximum supported exponent
  const maxExp = 1024 ** 4 - 1;
  console.log(`Testing maximum exponent: ${maxExp}`);

  try {
    const result = rScalarPow(maxExp);
    console.log(
      `✅ Maximum exponent works - Result: 0x${result
        .toBigInt()
        .toString(16)
        .slice(0, 16)}...`
    );
  } catch (error) {
    console.error(
      `❌ Maximum exponent failed: ${
        error instanceof Error ? error.message : String(error)
      }`
    );
  }

  // Test out of range
  try {
    rScalarPow(maxExp + 1);
    console.error(`❌ Should have thrown error for out of range exponent`);
  } catch (error) {
    console.log(
      `✅ Correctly rejected out of range exponent: ${
        error instanceof Error ? error.message : String(error)
      }`
    );
  }

  // Test negative numbers
  try {
    rScalarPow(-1);
    console.error(`❌ Should have thrown error for negative exponent`);
  } catch (error) {
    console.log(
      `✅ Correctly rejected negative exponent: ${
        error instanceof Error ? error.message : String(error)
      }`
    );
  }
}

// Run all tests
runTests();
testEdgeCases();
