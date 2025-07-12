/// Cross-platform verification test for large indices
/// <reference types="node" />

import { test, describe } from "node:test";
import assert from "node:assert";
import { Field, UInt32 } from "o1js";
import { rScalarPow } from "../src/exp.js";
import { scalar, digestStruct, commit, update } from "../src/commitment.js";

// Test specific indices with known correct values (computed from Move implementation)
const knownValues = [
  {
    index: 10,
    expected:
      "0x28b5085e0883923a5fe9bf9d108f8be3a432bd97156020d87dfce14ca429a152",
  },
  {
    index: 100,
    expected:
      "0x0a2d99b4b12e9eab0c9d3e52bd199c0467603f4c9b4a1563bcba82005a1dadd5",
  },
  {
    index: 1000,
    expected:
      "0x0a371b0d2bfc4852d08ee7fa75734e1bcddac0b1386718e2bf1c37947dda1af4",
  },
  {
    index: 1024,
    expected:
      "0x38d6066e4e230849541ab04bfcd0e0641952328eeeaa833da7bef93814e0e427",
  },
];

function testCrossPlatform(): void {
  console.log("🔄 Cross-platform verification for large indices...\n");

  let passed = 0;
  let failed = 0;

  for (const { index, expected } of knownValues) {
    try {
      const result = rScalarPow(index);
      const actual = "0x" + result.toBigInt().toString(16).padStart(64, "0");

      console.log(`Testing R^${index}:`);
      console.log(`  Expected: ${expected}`);
      console.log(`  Actual:   ${actual}`);

      if (actual === expected) {
        console.log(`  ✅ PASS\n`);
        passed++;
      } else {
        console.log(`  ❌ FAIL - Values don't match!\n`);
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

  console.log(`📊 Cross-platform Results:`);
  console.log(`✅ Passed: ${passed}`);
  console.log(`❌ Failed: ${failed}`);

  if (failed === 0) {
    console.log(`🎉 Perfect cross-platform consistency!`);
  } else {
    console.log(`⚠️  Cross-platform inconsistencies detected.`);
  }
}

testCrossPlatform();
