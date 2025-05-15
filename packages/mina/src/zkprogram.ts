import { ZkProgram, UInt32 } from "o1js";

const program = ZkProgram({
  name: "program",
  publicOutput: UInt32,
  methods: {
    check: {
      privateInputs: [UInt32],
      async method(data: UInt32) {
        return { publicOutput: data.add(1) };
      },
    },
  },
});

async function main() {
  for (let i = 0; i < 1000; i++) {
    const result = await program.rawMethods.check(UInt32.from(i));
  }
}

async function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main();
