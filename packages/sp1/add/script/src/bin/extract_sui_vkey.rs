use std::fs;

fn main() {
    // Read the SP1 v5 Groth16 verification key (this is a universal key for SP1, not program-specific)
    let vk_path =
        "/Users/mike/Documents/Silvana/Tests/sp1/sp1/crates/verifier/bn254-vk/groth16_vk.bin";

    match fs::read(vk_path) {
        Ok(vk_bytes) => {
            println!("SP1 v5 Groth16 VK (hex): 0x{}", hex::encode(&vk_bytes));
            println!("Length: {} bytes", vk_bytes.len());
        }
        Err(e) => {
            println!("Error reading verification key: {e}");
            println!("Make sure the SP1 repository is available at the expected path");
        }
    }
}
