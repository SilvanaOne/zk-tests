import { Field, ZkProgram, VerificationKey, Cache } from "o1js";
import os from "node:os";

const program = ZkProgram({
  name: "program",
  publicOutput: Field,
  methods: {
    add: {
      privateInputs: [Field],
      async method(value: Field) {
        value.assertLessThan(Field(100));
        return { publicOutput: value.add(Field(1)) };
      },
    },
  },
});

let vk: VerificationKey | null = null;

export async function processRequest(request: string): Promise<string> {
  try {
    if (!request.startsWith("proof-") && !request.startsWith("trace-")) {
      return "Wrong request, should be with proof-<number> or trace-<number> with number between 0 and 100";
    }

    const value = Number(request.split("-")[1]);
    console.log("value", value);
    if (request.startsWith("proof-")) {
      const usedMemoryBefore = os.totalmem() - os.freemem();
      const info = {
        cpuCores: os.cpus().length,
        totalMemory: os.totalmem().toLocaleString(),
        freeMemory: os.freemem().toLocaleString(),
        usedMemory: usedMemoryBefore.toLocaleString(),
        osVersion: os.version(),
        nodeVersion: process.version,
        architecture: os.arch(),
      };
      console.log("info", info);
      console.log("Proving...");

      if (!vk) {
        console.time("compiled");
        const cache = Cache.FileSystem("./cache");
        vk = (await program.compile({ cache })).verificationKey;
        console.timeEnd("compiled");
      }
      console.time("proved");
      const result = await program.add(Field(value));
      console.timeEnd("proved");
      const usedMemoryAfter = os.totalmem() - os.freemem();
      const infoAfter = {
        freeMemory: os.freemem().toLocaleString(),
        usedMemory: usedMemoryAfter.toLocaleString(),
        additionalMemory: (usedMemoryAfter - usedMemoryBefore).toLocaleString(),
      };
      console.log("info after", infoAfter);
      return `proved result: ${
        result?.proof?.publicOutput?.toJSON() ?? "failed"
      }`;
    } else {
      console.log("Tracing...");
      console.time("traced");
      const result = await program.rawMethods.add(Field(value));
      console.timeEnd("traced");
      return `trace result: ${result?.publicOutput?.toJSON() ?? "failed"}`;
    }
  } catch (error) {
    return "catch: wrong request, should be with proof-<number> or trace-<number> with number between 0 and 100";
  }
}
