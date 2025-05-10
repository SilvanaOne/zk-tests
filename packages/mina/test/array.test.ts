import { describe, it } from "node:test";
import assert from "node:assert";
import {
  Mina,
  PrivateKey,
  DynamicProof,
  VerificationKey,
  Void,
  ZkProgram,
  Field,
  SmartContract,
  method,
  AccountUpdate,
  state,
  State,
  Cache,
  FeatureFlags,
  verify,
  PublicKey,
  Bool,
  Provable,
  UInt8,
  UInt32,
} from "o1js";

const COUNT = 100;

const program = ZkProgram({
  name: "program",
  publicOutput: Provable.Array(UInt8, COUNT),
  methods: {
    check: {
      privateInputs: [Provable.Array(UInt8, COUNT), UInt32],
      async method(data: UInt8[], idx: UInt32) {
        let n255Filter = UInt8.from(0);
        const twoFiftyFive = UInt8.from(255);
        let is255 = Bool(false);
        let indexBeforePhoto = Bool(false);
        let is255AndIndexBeforePhoto = Bool(false);
        for (let i = 0; i < COUNT; i++) {
          is255 = data[i].value.equals(twoFiftyFive.value);

          indexBeforePhoto = UInt32.from(i).lessThan(idx);
          is255AndIndexBeforePhoto = Bool.and(is255, indexBeforePhoto);

          const n255FilterDelta = Provable.if(
            is255AndIndexBeforePhoto,
            Field(1),
            Field(0)
          );

          n255Filter = n255Filter.add(UInt8.from(n255FilterDelta));

          // const dataDelta = Provable.if(
          //   is255AndIndexBeforePhoto,
          //   n255Filter.value,
          //   Field(0)
          // );
          // data[i] = data[i].add(UInt8.from(dataDelta));
          data[i] = data[i].add(UInt8.from(n255Filter));
        }
        return { publicOutput: data };
      },
    },
  },
});

describe("Arrya", () => {
  it("should calculate number of constraints", async () => {
    const methods = await program.analyzeMethods();
    console.log((methods as any).check.rows);
  });
});
