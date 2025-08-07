#![no_main]
sp1_zkvm::entrypoint!(main);

use proof_lib::deserialize_poseidon_proof_zkvm;

pub fn main() {
    // Read the serialized proof bytes
    let proof_bytes = sp1_zkvm::io::read::<Vec<u8>>();
    
    // Test deserialization only using zkVM-friendly version
    // This version doesn't require SRS files
    // We'll measure the cycles required for deserialization
    
    let mut deserialized_successfully = false;
    let mut error_code = 0u32; // 0 = success, 1 = deserialize failed
    
    // Attempt to deserialize the proof
    match deserialize_poseidon_proof_zkvm(&proof_bytes) {
        Ok(()) => {
            // Successfully deserialized
            deserialized_successfully = true;
            error_code = 0;
        }
        Err(_) => {
            // Deserialization error
            deserialized_successfully = false;
            error_code = 1;
        }
    }
    
    // Commit the deserialization result
    sp1_zkvm::io::commit(&deserialized_successfully);
    
    // Commit error code for debugging
    sp1_zkvm::io::commit(&error_code);
    
    // Also commit proof size for debugging
    sp1_zkvm::io::commit(&(proof_bytes.len() as u32));
}
