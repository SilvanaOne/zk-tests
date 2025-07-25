import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { F } from "../target/types/f";

describe("poseidon", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.poseidon as Program<F>;

  it("Is initialized!", async () => {
    // Add your test here.
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  });

  it("Can add two numbers using poseidon", async () => {
    // Test the poseidon function with two numbers
    const a = new anchor.BN(1);
    const b = new anchor.BN(2);
    const count = new anchor.BN(4);
    
    // Call the poseidon function with the test values and pass options to set higher CU limit
    console.time("Poseidon transaction");
    const tx = await program.methods.poseidon(a, b, count)
      .preInstructions([
        {
          programId: new anchor.web3.PublicKey("ComputeBudget111111111111111111111111111111"),
          keys: [],
          data: anchor.web3.ComputeBudgetProgram.requestHeapFrame({ 
            bytes: 256 * 1024 // Request 256KB of heap
          }).data
        },
        {
          programId: new anchor.web3.PublicKey("ComputeBudget111111111111111111111111111111"),
          keys: [],
          data: anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({ 
            units: 1_000_000_000
          }).data
        }

      ])
      .rpc();
    
    console.timeEnd("Poseidon transaction");
    console.log("Poseidon transaction signature", tx);
    
  });
});
