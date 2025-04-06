import { describe, it } from "node:test";
import assert from "node:assert";
import os from "node:os";
describe("Get info", async () => {
  it("should get info", async () => {
    const info = {
      cpuCores: os.cpus().length,
      totalMemory: os.totalmem(),
      freeMemory: os.freemem(),
      memoryUsage: process.memoryUsage(),
      platform: os.platform(),
      release: os.release(),
      type: os.type(),
      version: os.version(),
      hostname: os.hostname(),
      userInfo: os.userInfo(),
      arch: os.arch(),
      tmpdir: os.tmpdir(),
      homedir: os.homedir(),
    };
    console.log(info);
  });
  it("should get short info", async () => {
    const info = {
      cpuCores: os.cpus().length,
      totalMemory: os.totalmem(),
      architecture: os.arch(),
    };
    console.log(info);
  });
});
