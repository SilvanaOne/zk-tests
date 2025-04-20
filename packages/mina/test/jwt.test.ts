import { describe, it } from "node:test";
import assert from "node:assert";
import * as api from "@silvana-one/api";
import { SignJWT, jwtVerify } from "jose";

describe("JWT", () => {
  it(`should verify JWT`, async () => {
    const result = await jwtVerify(
      process.env.JWT!,
      new TextEncoder().encode(process.env.JWT_PRIVATE_KEY!)
    );
    console.log(result?.payload?.id);
  });
});
