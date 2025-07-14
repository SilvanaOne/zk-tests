import { test, describe } from "node:test";
import assert from "node:assert";
import { Encoding } from "o1js";
import { Fr } from "../src/constants.js";

describe("Strings conversion", () => {
  test("should convert a string to fields and back", () => {
    const text = "Hello, world!";
    const fields = Encoding.stringToFields(text);
    assert.strictEqual(fields.length, 1, "Should be 1 field");
    const field = fields[0];
    const stringField = field.toBigInt();
    console.log("encoded:", stringField);
    assert.strictEqual(stringField, 22928018571998998000425702810952n);
    const text2 = Encoding.stringFromFields(fields);
    assert.strictEqual(text, text2);
  });
  test("should convert a field to bls Scalar", () => {
    const scalar = Fr.from(22928018571998998000425702810952n);
    console.log("scalar:", scalar.toBigInt());
    console.log("scalar:", scalar.toBigInt().toString(16));
  });
});
