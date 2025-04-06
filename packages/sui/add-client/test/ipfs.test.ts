import { describe, it } from "node:test";
import assert from "node:assert";
import { PinataSDK } from "pinata";
import "dotenv/config";

const pinata = new PinataSDK({
  pinataJwt: process.env.PINATA_JWT,
  pinataGateway: process.env.GATEWAY_URL,
  pinataGatewayKey: process.env.GATEWAY_API_KEY,
});

let blobId: string | undefined =
  "bafkreifzjut3te2nhyekklss27nh3k72ysco7y32koao5eei66wof36n5e";

describe("IPFS test", async () => {
  it("should upload to ipfs", async () => {
    try {
      const file = new File(["hello world"], "Testing.txt", {
        type: "text/plain",
      });
      const upload = await pinata.upload.public.file(file);
      console.log(upload);
      blobId = upload.cid;
    } catch (error) {
      console.log(error);
    }
  });
  it("should read from ipfs", async () => {
    if (!blobId) {
      throw new Error("blobId is not set");
    }
    try {
      const data = await pinata.gateways.public.get(blobId);
      console.log(data);

      const url = await pinata.gateways.public.convert(blobId);
      console.log(url);
    } catch (error) {
      console.log(error);
    }
  });
});
