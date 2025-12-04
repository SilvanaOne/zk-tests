import { test } from "node:test";
import assert from "node:assert";
import { fromBinary } from "@bufbuild/protobuf";
import { FatContractInstanceSchema, VersionedSchema } from "../src/proto/com/digitalasset/daml/lf/transaction_pb";
import { VersionedValueSchema, ValueSchema } from "../src/proto/com/digitalasset/daml/lf/value_pb";
import { decodeCIP56HoldingBlob, decodeContractBlob } from "../src/lib/blob";

// Test data from actual Loop SDK responses
const AMULET_BLOB = "CgMyLjESnQUKRQCC9q29WLsBZGnjWzpCwp0pK6q7aOpd80NHdy2a1ecYjsoSEiAql7wXidazjNo+Or40RSUphhDkAW91v7FIn9r+pax31BINc3BsaWNlLWFtdWxldBpaCkAzY2ExMzQzYWIyNmI0NTNkMzhjOGFkYjcwZGNhNWYxZWFkODQ0MGM0MmI1OWI2OGYwNzA3ODY5NTVjYmY5ZWMxEgZTcGxpY2USBkFtdWxldBoGQW11bGV0IoACav0BCk0KSzpJRFNPOjoxMjIwYmU1OGMyOWU2NWRlNDBiZjI3M2JlMWRjMmIyNjZkNDNhOWEwMDJlYTViMTg5NTVhZWVmN2FhYzg4MWJiNDcxYQpqCmg6ZjFlNmI5ZTMwZGUwM2M4ZGYyYzBiNWY1YzUwY2NhYjE3OjoxMjIwZmYyNWEzZTk3NmJlNGJlMmM2OWI1NzYyNWEwOGQ5N2Y4NDdiZjI0NzM2NmFjZDcwZWRjY2NlYjI5N2I3ZTU4MwpACj5qPAoUChIyEDE5MjY3Ljk4MDAwMDAwMDAKDAoKaggKBgoEGO7GAgoWChRqEgoQCg4yDDAuMDAwMjE1NjY0MipmMWU2YjllMzBkZTAzYzhkZjJjMGI1ZjVjNTBjY2FiMTc6OjEyMjBmZjI1YTNlOTc2YmU0YmUyYzY5YjU3NjI1YTA4ZDk3Zjg0N2JmMjQ3MzY2YWNkNzBlZGNjY2ViMjk3YjdlNTgzKklEU086OjEyMjBiZTU4YzI5ZTY1ZGU0MGJmMjczYmUxZGMyYjI2NmQ0M2E5YTAwMmVhNWIxODk1NWFlZWY3YWFjODgxYmI0NzFhOR0aIwmrRAYAQioKJgokCAESIITVOaoDU0C6eP7HOI2X1F8FsFvuUdV5H1jOQepijglQEB4=";

const CIP56_HOLDING_BLOB = "CgMyLjES5AgKRQBED6HK3FfzRJRj3dxrEWM/Lf42AzpUv4Lw/GWYgOCQ5coSEiBQOCyDp/MDZnrGtZbDCTKXqjHra2mg/raLR8xOK4z3thIbdXRpbGl0eS1yZWdpc3RyeS1ob2xkaW5nLXYwGnQKQGRkM2E5ZjJkNTFjYzRjNTJkOWVjMmUxZDdmZjIzNTI5OGRjZmIzYWZkMWQ1MGFiNDQzMjhiMWFhYTlhMTg1ODcSB1V0aWxpdHkSCFJlZ2lzdHJ5EgdIb2xkaW5nEgJWMBIHSG9sZGluZxoHSG9sZGluZyKsBGqpBApsCmo6aGF1dGgwXzAwN2M2NWY4NTdmMWMzZDU5OWNiNmRmNzM3NzU6OjEyMjBkMmQ3MzJkMDQyYzI4MWNlZTgwZjQ4M2FiODBmM2NiYWE0NzgyODYwZWQ1ZjRkYzIyOGFiMDNkZWRkMmVlOGY5ClYKVDpSdGVzdC10b2tlbi0xOjoxMjIwMzRmYWY4ZjRhZjcxZDEwN2E0MjQ0MWY4YmM5MGNhYmZkNjNhYjQzODZmYzdmMTdkMTVkNmUzYjAxYzViZDJhZQpWClQ6UnRlc3QtdG9rZW4tMTo6MTIyMDM0ZmFmOGY0YWY3MWQxMDdhNDI0NDFmOGJjOTBjYWJmZDYzYWI0Mzg2ZmM3ZjE3ZDE1ZDZlM2IwMWM1YmQyYWUKagpoOmYxZTZiOWUzMGRlMDNjOGRmMmMwYjVmNWM1MGNjYWIxNzo6MTIyMGZmMjVhM2U5NzZiZTRiZTJjNjliNTc2MjVhMDhkOTdmODQ3YmYyNDczNjZhY2Q3MGVkY2NjZWIyOTdiN2U1ODMKhAEKgQFqfwpWClQ6UnRlc3QtdG9rZW4tMTo6MTIyMDM0ZmFmOGY0YWY3MWQxMDdhNDI0NDFmOGJjOTBjYWJmZDYzYWI0Mzg2ZmM3ZjE3ZDE1ZDZlM2IwMWM1YmQyYWUKCAoGQgRXQlRDChsKGUIXUmVnaXN0cmFySW50ZXJuYWxTY2hlbWUKBAoCQgAKEAoOMgwxLjAwMDAwMDAwMDAqZjFlNmI5ZTMwZGUwM2M4ZGYyYzBiNWY1YzUwY2NhYjE3OjoxMjIwZmYyNWEzZTk3NmJlNGJlMmM2OWI1NzYyNWEwOGQ5N2Y4NDdiZjI0NzM2NmFjZDcwZWRjY2NlYjI5N2I3ZTU4MypSdGVzdC10b2tlbi0xOjoxMjIwMzRmYWY4ZjRhZjcxZDEwN2E0MjQ0MWY4YmM5MGNhYmZkNjNhYjQzODZmYzdmMTdkMTVkNmUzYjAxYzViZDJhZTJoYXV0aDBfMDA3YzY1Zjg1N2YxYzNkNTk5Y2I2ZGY3Mzc3NTo6MTIyMGQyZDczMmQwNDJjMjgxY2VlODBmNDgzYWI4MGYzY2JhYTQ3ODI4NjBlZDVmNGRjMjI4YWIwM2RlZGQyZWU4Zjk5CVg3i75EBgBCKgomCiQIARIgCrLMkkQ8q5Cp/2cuPuahuCCNjcDAi/STZeTG1X3z5F8QHg==";

// Low-level test to understand the protobuf structure
test("Low-level protobuf parsing - Versioned wrapper", () => {
  const binaryData = Buffer.from(AMULET_BLOB, "base64");
  console.log("Binary data length:", binaryData.length);
  console.log("First 20 bytes (hex):", binaryData.subarray(0, 20).toString("hex"));

  const versioned = fromBinary(VersionedSchema, binaryData);
  console.log("Versioned.version:", versioned.version);
  console.log("Versioned.payload length:", versioned.payload?.length);
  console.log("Versioned.payload first 20 bytes (hex):", Buffer.from(versioned.payload).subarray(0, 20).toString("hex"));

  assert.strictEqual(versioned.version, "2.1");
  assert.ok(versioned.payload.length > 0);
});

test("Low-level protobuf parsing - FatContractInstance", () => {
  const binaryData = Buffer.from(AMULET_BLOB, "base64");
  const versioned = fromBinary(VersionedSchema, binaryData);

  const fatContract = fromBinary(FatContractInstanceSchema, versioned.payload);
  console.log("FatContract.packageName:", fatContract.packageName);
  console.log("FatContract.templateId:", fatContract.templateId);
  console.log("FatContract.createArg length:", fatContract.createArg?.length);
  console.log("FatContract.createArg first 20 bytes (hex):", Buffer.from(fatContract.createArg).subarray(0, 20).toString("hex"));

  assert.strictEqual(fatContract.packageName, "splice-amulet");
  assert.ok(fatContract.templateId);
  assert.ok(fatContract.createArg.length > 0);
});

test("Low-level protobuf parsing - VersionedValue (create_arg)", () => {
  const binaryData = Buffer.from(AMULET_BLOB, "base64");
  const versioned = fromBinary(VersionedSchema, binaryData);
  const fatContract = fromBinary(FatContractInstanceSchema, versioned.payload);

  console.log("createArg raw (hex):", Buffer.from(fatContract.createArg).toString("hex"));

  // Try parsing as VersionedValue
  try {
    const versionedValue = fromBinary(VersionedValueSchema, fatContract.createArg);
    console.log("VersionedValue.version:", versionedValue.version);
    console.log("VersionedValue.value length:", versionedValue.value?.length);
    console.log("VersionedValue.value (hex):", Buffer.from(versionedValue.value).toString("hex"));
  } catch (e) {
    console.log("Failed to parse as VersionedValue:", e);
  }

  // Try parsing directly as Value (maybe it's not wrapped)
  try {
    const value = fromBinary(ValueSchema, fatContract.createArg);
    console.log("Direct Value.sum.case:", value.sum.case);
    if (value.sum.case === "record") {
      console.log("Record fields count:", value.sum.value.fields.length);
      value.sum.value.fields.forEach((f, i) => {
        console.log(`  Field ${i}: case=${f.value?.sum.case}`);
      });
    }
  } catch (e) {
    console.log("Failed to parse directly as Value:", e);
  }
});

test("decodeContractBlob - Amulet blob structure", () => {
  const decoded = decodeContractBlob(AMULET_BLOB);

  console.log("Amulet decoded:", JSON.stringify(decoded, (_, v) =>
    v instanceof Uint8Array ? `Uint8Array(${v.length})` :
    typeof v === "bigint" ? v.toString() : v, 2));

  assert.ok(decoded, "Should decode successfully");
  assert.strictEqual(decoded.packageName, "splice-amulet");
  assert.ok(decoded.templateId, "Should have templateId");
  assert.deepStrictEqual(decoded.templateId?.name, ["Amulet"]);
  assert.ok(decoded.fields, "Should have fields");

  console.log("Fields count:", decoded.fields?.fields.length);
  console.log("Field types:", decoded.fields?.fields.map((f, i) =>
    `${i}: ${f.value?.sum.case}`));
});

test("decodeContractBlob - CIP-56 Holding blob structure", () => {
  const decoded = decodeContractBlob(CIP56_HOLDING_BLOB);

  console.log("CIP-56 decoded:", JSON.stringify(decoded, (_, v) =>
    v instanceof Uint8Array ? `Uint8Array(${v.length})` : v, 2));

  assert.ok(decoded, "Should decode successfully");
  assert.strictEqual(decoded.packageName, "utility-registry-holding-v0");
  assert.ok(decoded.templateId, "Should have templateId");
  assert.deepStrictEqual(decoded.templateId?.name, ["Holding"]);
  assert.ok(decoded.fields, "Should have fields");

  console.log("Fields count:", decoded.fields?.fields.length);
  console.log("Field types:", decoded.fields?.fields.map((f, i) =>
    `${i}: ${f.value?.sum.case}`));
});

test("decodeCIP56HoldingBlob - Amulet contract", () => {
  const result = decodeCIP56HoldingBlob(AMULET_BLOB);

  console.log("Amulet as CIP-56:", JSON.stringify(result, null, 2));

  assert.ok(result, "Should decode successfully");
  assert.strictEqual(result.instrument.id, "CC", "Should map to Canton Coin");
  assert.ok(parseFloat(result.amount) > 0, "Should have non-zero amount");
  assert.ok(result.owner.length > 0, "Should have owner");
});

test("decodeCIP56HoldingBlob - Utility Registry Holding", () => {
  const result = decodeCIP56HoldingBlob(CIP56_HOLDING_BLOB);

  console.log("CIP-56 Holding:", JSON.stringify(result, null, 2));

  assert.ok(result, "Should decode successfully");
  assert.ok(result.instrument.id.length > 0, "Should have instrument ID");
  assert.ok(parseFloat(result.amount) > 0, "Should have non-zero amount");
  assert.ok(result.owner.length > 0, "Should have owner");
});
