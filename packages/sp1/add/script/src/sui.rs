use ark_bn254::{Bn254, Fr};
use ark_groth16::{Groth16, Proof, VerifyingKey, prepare_verifying_key};
use ark_serialize::CanonicalSerialize;
use ark_snark::SNARK;
use num_bigint::BigUint;
use num_traits::Num;
use sp1_sdk::SP1ProofWithPublicValues;

/// Struct for Sui proof conversion result
#[derive(Debug)]
pub struct SuiProofData {
    pub vkey_bytes: Vec<u8>,
    pub public_inputs_bytes: Vec<u8>,
    pub proof_bytes: Vec<u8>,
}

// SP1 v5 verification key bytes - load from environment or hardcode the bytes
const GROTH16_VK_BYTES: &[u8] = &[
    0xad, 0x4d, 0x9a, 0xa7, 0xe3, 0x02, 0xd9, 0xdf, 0x41, 0x74, 0x9d, 0x55, 0x07, 0x94, 0x9d, 0x05,
    0xdb, 0xea, 0x33, 0xfb, 0xb1, 0x6c, 0x64, 0x3b, 0x22, 0xf5, 0x99, 0xa2, 0xbe, 0x6d, 0xf2, 0xe2,
    0xe1, 0xa1, 0x57, 0x5c, 0x2e, 0x49, 0x4d, 0x36, 0x13, 0xe9, 0x5e, 0x43, 0xb6, 0x22, 0x31, 0x8d,
    0x92, 0x25, 0xc8, 0x20, 0xe4, 0x6a, 0xcd, 0x08, 0xe8, 0xc9, 0x87, 0xb4, 0x40, 0x51, 0x19, 0x5b,
    0xc9, 0x67, 0x03, 0x2f, 0xcb, 0xf7, 0x76, 0xd1, 0xaf, 0xc9, 0x85, 0xf8, 0x88, 0x77, 0xf1, 0x82,
    0xd3, 0x84, 0x80, 0xa6, 0x53, 0xf2, 0xde, 0xca, 0xa9, 0x79, 0x4c, 0xbc, 0x3b, 0xf3, 0x06, 0x0c,
    0x0e, 0x18, 0x78, 0x47, 0xad, 0x4c, 0x79, 0x83, 0x74, 0xd0, 0xd6, 0x73, 0x2b, 0xf5, 0x01, 0x84,
    0x7d, 0xd6, 0x8b, 0xc0, 0xe0, 0x71, 0x24, 0x1e, 0x02, 0x13, 0xbc, 0x7f, 0xc1, 0x3d, 0xb7, 0xab,
    0x99, 0x8e, 0x93, 0x93, 0x92, 0x0d, 0x48, 0x3a, 0x72, 0x60, 0xbf, 0xb7, 0x31, 0xfb, 0x5d, 0x25,
    0xf1, 0xaa, 0x49, 0x33, 0x35, 0xa9, 0xe7, 0x12, 0x97, 0xe4, 0x85, 0xb7, 0xae, 0xf3, 0x12, 0xc2,
    0x18, 0x00, 0xde, 0xef, 0x12, 0x1f, 0x1e, 0x76, 0x42, 0x6a, 0x00, 0x66, 0x5e, 0x5c, 0x44, 0x79,
    0x67, 0x43, 0x22, 0xd4, 0xf7, 0x5e, 0xda, 0xdd, 0x46, 0xde, 0xbd, 0x5c, 0xd9, 0x92, 0xf6, 0xed,
    0xd8, 0xe5, 0x73, 0x9a, 0x73, 0xd6, 0x57, 0xe8, 0x32, 0xa3, 0x36, 0x79, 0x19, 0x77, 0x33, 0x2a,
    0x4b, 0x96, 0xe5, 0xbb, 0xfd, 0xcb, 0x99, 0x03, 0xaf, 0xe4, 0x87, 0xdb, 0x9a, 0xa6, 0xcb, 0x5d,
    0xdc, 0xc7, 0xcb, 0x8d, 0xe7, 0x15, 0x67, 0x5f, 0x21, 0xf0, 0x1e, 0xcc, 0x9b, 0x46, 0xd2, 0x36,
    0xe0, 0x86, 0x5e, 0x0c, 0xc0, 0x20, 0x02, 0x45, 0x21, 0x99, 0x82, 0x69, 0x84, 0x5f, 0x74, 0xe6,
    0x03, 0xff, 0x41, 0xf4, 0xba, 0x0c, 0x37, 0xfe, 0x2c, 0xaf, 0x27, 0x35, 0x4d, 0x28, 0xe4, 0xb8,
    0xf8, 0x3d, 0x3b, 0x76, 0x77, 0x7a, 0x63, 0xb3, 0x27, 0xd7, 0x36, 0xbf, 0xfb, 0x01, 0x22, 0xed,
    0x00, 0x00, 0x00, 0x03, 0xa6, 0x09, 0x1e, 0x1c, 0xaf, 0xb0, 0xad, 0x8a, 0x4e, 0xa0, 0xa6, 0x94,
    0xcd, 0x37, 0x43, 0xeb, 0xf5, 0x24, 0x77, 0x92, 0x33, 0xdb, 0x73, 0x4c, 0x45, 0x1d, 0x28, 0xb5,
    0x8a, 0xa9, 0x75, 0x8e, 0x86, 0x1c, 0x3f, 0xd0, 0xfd, 0x3d, 0xa2, 0x5d, 0x26, 0x07, 0xc2, 0x27,
    0xd0, 0x90, 0xcc, 0xa7, 0x50, 0xed, 0x36, 0xc6, 0xec, 0x87, 0x87, 0x55, 0xe5, 0x37, 0xc1, 0xc4,
    0x89, 0x51, 0xfb, 0x4c, 0x84, 0xea, 0xb2, 0x41, 0x38, 0x8a, 0x79, 0x81, 0x7f, 0xe0, 0xe0, 0xe2,
    0xea, 0xd0, 0xb2, 0xec, 0x4f, 0xfd, 0xec, 0x51, 0xa1, 0x60, 0x28, 0xde, 0xe0, 0x20, 0x63, 0x4f,
    0xd1, 0x29, 0xe7, 0x1c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

// Re-export the conversion utilities from SP1-Sui
pub use ark_converter::*;

mod ark_converter {
    use super::*;
    use ark_bn254::{G1Affine, G2Affine};
    use ark_ff::PrimeField;
    use ark_serialize::{CanonicalDeserialize, Compress, Validate};

    pub const GNARK_MASK: u8 = 0b11 << 6;
    pub const GNARK_COMPRESSED_POSITIVE: u8 = 0b10 << 6;
    pub const GNARK_COMPRESSED_NEGATIVE: u8 = 0b11 << 6;
    pub const GNARK_COMPRESSED_INFINITY: u8 = 0b01 << 6;

    pub const ARK_MASK: u8 = 0b11 << 6;
    pub const ARK_COMPRESSED_POSITIVE: u8 = 0b00 << 6;
    pub const ARK_COMPRESSED_NEGATIVE: u8 = 0b10 << 6;
    pub const ARK_COMPRESSED_INFINITY: u8 = 0b01 << 6;

    #[derive(Debug)]
    pub enum ArkGroth16Error {
        G1CompressionError,
        G2CompressionError,
        InvalidInput,
    }

    impl std::fmt::Display for ArkGroth16Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ArkGroth16Error::G1CompressionError => write!(f, "G1 compression error"),
                ArkGroth16Error::G2CompressionError => write!(f, "G2 compression error"),
                ArkGroth16Error::InvalidInput => write!(f, "Invalid input"),
            }
        }
    }

    impl std::error::Error for ArkGroth16Error {}

    /// Convert the endianness of a byte array, chunk by chunk.
    pub fn convert_endianness<const CHUNK_SIZE: usize, const ARRAY_SIZE: usize>(
        bytes: &[u8; ARRAY_SIZE],
    ) -> [u8; ARRAY_SIZE] {
        let reversed: [_; ARRAY_SIZE] = bytes
            .chunks_exact(CHUNK_SIZE)
            .flat_map(|chunk| chunk.iter().rev().copied())
            .enumerate()
            .fold([0u8; ARRAY_SIZE], |mut acc, (i, v)| {
                acc[i] = v;
                acc
            });
        reversed
    }

    pub fn gnark_flag_to_ark_flag(msb: u8) -> Result<u8, ArkGroth16Error> {
        let gnark_flag = msb & GNARK_MASK;

        let ark_flag = match gnark_flag {
            GNARK_COMPRESSED_POSITIVE => ARK_COMPRESSED_POSITIVE,
            GNARK_COMPRESSED_NEGATIVE => ARK_COMPRESSED_NEGATIVE,
            GNARK_COMPRESSED_INFINITY => ARK_COMPRESSED_INFINITY,
            _ => {
                return Err(ArkGroth16Error::InvalidInput);
            }
        };

        Ok(msb & !ARK_MASK | ark_flag)
    }

    pub fn gnark_compressed_x_to_ark_compressed_x(x: &[u8]) -> Result<Vec<u8>, ArkGroth16Error> {
        if x.len() != 32 && x.len() != 64 {
            return Err(ArkGroth16Error::InvalidInput);
        }
        let mut x_copy = x.to_owned();

        let msb = gnark_flag_to_ark_flag(x_copy[0])?;
        x_copy[0] = msb;

        x_copy.reverse();
        Ok(x_copy)
    }

    /// Decompress a G1 point.
    pub fn decompress_g1(g1_bytes: &[u8; 32]) -> Result<G1Affine, ArkGroth16Error> {
        let g1_bytes = gnark_compressed_x_to_ark_compressed_x(g1_bytes)?;
        let g1_bytes = convert_endianness::<32, 32>(&g1_bytes.as_slice().try_into().unwrap());
        let decompressed_g1 = G1Affine::deserialize_with_mode(
            convert_endianness::<32, 32>(&g1_bytes).as_slice(),
            Compress::Yes,
            Validate::No,
        )
        .map_err(|_| ArkGroth16Error::G1CompressionError)?;
        Ok(decompressed_g1)
    }

    /// Decompress a G2 point.
    pub fn decompress_g2(g2_bytes: &[u8; 64]) -> Result<G2Affine, ArkGroth16Error> {
        let g2_bytes = gnark_compressed_x_to_ark_compressed_x(g2_bytes)?;
        let g2_bytes = convert_endianness::<64, 64>(&g2_bytes.as_slice().try_into().unwrap());
        let decompressed_g2 = G2Affine::deserialize_with_mode(
            convert_endianness::<64, 64>(&g2_bytes).as_slice(),
            Compress::Yes,
            Validate::No,
        )
        .map_err(|_| ArkGroth16Error::G2CompressionError)?;
        Ok(decompressed_g2)
    }

    /// Deserialize a gnark decompressed affine G1 point to an arkworks decompressed affine G1 point.
    pub fn gnark_decompressed_g1_to_ark_decompressed_g1(
        buf: &[u8; 64],
    ) -> Result<G1Affine, ArkGroth16Error> {
        let buf = convert_endianness::<32, 64>(buf);
        if buf == [0u8; 64] {
            return Ok(G1Affine::identity());
        }
        let g1 = G1Affine::deserialize_with_mode(
            &*[&buf[..], &[0u8][..]].concat(),
            Compress::No,
            Validate::Yes,
        )
        .map_err(|_| ArkGroth16Error::G1CompressionError)?;
        Ok(g1)
    }

    /// Deserialize a gnark decompressed affine G2 point to an arkworks decompressed affine G2 point.
    pub fn gnark_decompressed_g2_to_ark_decompressed_g2(
        buf: &[u8; 128],
    ) -> Result<G2Affine, ArkGroth16Error> {
        let buf = convert_endianness::<64, 128>(buf);
        if buf == [0u8; 128] {
            return Ok(G2Affine::identity());
        }
        let g2 = G2Affine::deserialize_with_mode(
            &*[&buf[..], &[0u8][..]].concat(),
            Compress::No,
            Validate::Yes,
        )
        .map_err(|_| ArkGroth16Error::G2CompressionError)?;
        Ok(g2)
    }

    /// Load a Groth16 proof from bytes in the arkworks format.
    pub fn load_ark_proof_from_bytes(buffer: &[u8]) -> Result<Proof<Bn254>, ArkGroth16Error> {
        Ok(Proof::<Bn254> {
            a: gnark_decompressed_g1_to_ark_decompressed_g1(buffer[..64].try_into().unwrap())?,
            b: gnark_decompressed_g2_to_ark_decompressed_g2(buffer[64..192].try_into().unwrap())?,
            c: gnark_decompressed_g1_to_ark_decompressed_g1(&buffer[192..256].try_into().unwrap())?,
        })
    }

    /// Load a Groth16 verifying key from bytes in the arkworks format.
    pub fn load_ark_groth16_verifying_key_from_bytes(
        buffer: &[u8],
    ) -> Result<VerifyingKey<Bn254>, ArkGroth16Error> {
        let alpha_g1 = decompress_g1(buffer[..32].try_into().unwrap())?;
        let beta_g2 = decompress_g2(buffer[64..128].try_into().unwrap())?;
        let gamma_g2 = decompress_g2(buffer[128..192].try_into().unwrap())?;
        let delta_g2 = decompress_g2(buffer[224..288].try_into().unwrap())?;

        let num_k = u32::from_be_bytes([buffer[288], buffer[289], buffer[290], buffer[291]]);
        let mut k = Vec::new();
        let mut offset = 292;
        for _ in 0..num_k {
            let point = decompress_g1(&buffer[offset..offset + 32].try_into().unwrap())?;
            k.push(point);
            offset += 32;
        }

        let num_of_array_of_public_and_commitment_committed = u32::from_be_bytes([
            buffer[offset],
            buffer[offset + 1],
            buffer[offset + 2],
            buffer[offset + 3],
        ]);
        offset += 4;
        for _ in 0..num_of_array_of_public_and_commitment_committed {
            let num = u32::from_be_bytes([
                buffer[offset],
                buffer[offset + 1],
                buffer[offset + 2],
                buffer[offset + 3],
            ]);
            offset += 4;
            for _ in 0..num {
                offset += 4;
            }
        }

        Ok(VerifyingKey {
            alpha_g1,
            beta_g2,
            gamma_g2,
            delta_g2,
            gamma_abc_g1: k,
        })
    }

    /// Load the public inputs from the bytes in the arkworks format.
    pub fn load_ark_public_inputs_from_bytes(
        vkey_hash: &[u8; 32],
        committed_values_digest: &[u8; 32],
    ) -> [Fr; 2] {
        [
            Fr::from_be_bytes_mod_order(vkey_hash),
            Fr::from_be_bytes_mod_order(committed_values_digest),
        ]
    }
}

/// Convert SP1 proof to Sui-compatible format
pub fn convert_sp1_proof_for_sui(
    sp1_proof_with_public_values: SP1ProofWithPublicValues,
) -> Result<SuiProofData, Box<dyn std::error::Error>> {
    let proof_bytes = sp1_proof_with_public_values.bytes();

    let proof = sp1_proof_with_public_values
        .proof
        .try_as_groth_16()
        .ok_or("Failed to convert proof to Groth16 proof")?;

    // Convert vkey hash to bytes
    let vkey_hash = BigUint::from_str_radix(&proof.public_inputs[0], 10)
        .map_err(|e| format!("Failed to parse vkey hash: {e}"))?
        .to_bytes_be();

    // To match the standard format, the 31 byte vkey hash is left padded with a 0 byte
    let mut padded_vkey_hash = vec![0];
    padded_vkey_hash.extend_from_slice(&vkey_hash);
    let vkey_hash = padded_vkey_hash;

    // Load and process Groth16 verification key
    let ark_groth16_vk = load_ark_groth16_verifying_key_from_bytes(GROTH16_VK_BYTES)
        .map_err(|e| format!("Failed to load verification key: {e}"))?;

    // Serialize verification key
    let mut ark_groth16_serialized = Vec::new();
    ark_groth16_vk
        .serialize_compressed(&mut ark_groth16_serialized)
        .map_err(|e| format!("Failed to serialize verification key: {e}"))?;

    // Process proof points
    let ark_proof = load_ark_proof_from_bytes(&proof_bytes[4..])
        .map_err(|e| format!("Failed to load proof: {e}"))?;
    let mut ark_proof_serialized = Vec::new();
    ark_proof
        .serialize_compressed(&mut ark_proof_serialized)
        .map_err(|e| format!("Failed to serialize proof: {e}"))?;

    // Process public inputs
    let mut ark_padded_vkey_hash: [u8; 32] = [0u8; 32];
    ark_padded_vkey_hash[..vkey_hash.len()].copy_from_slice(&vkey_hash);

    let committed_values_digest = BigUint::from_str_radix(&proof.public_inputs[1], 10)
        .map_err(|e| format!("Failed to parse committed values digest: {e}"))?
        .to_bytes_be();
    let mut padded_committed_values_digest = [0u8; 32];
    padded_committed_values_digest[..committed_values_digest.len()]
        .copy_from_slice(&committed_values_digest);

    let ark_public_inputs =
        load_ark_public_inputs_from_bytes(&ark_padded_vkey_hash, &padded_committed_values_digest);
    let mut ark_public_inputs_serialized = Vec::new();
    ark_public_inputs.iter().for_each(|input| {
        input
            .serialize_compressed(&mut ark_public_inputs_serialized)
            .unwrap();
    });

    // Verify the proof (optional check)
    let ark_pvk = prepare_verifying_key(&ark_groth16_vk);
    let ark_verified =
        Groth16::<Bn254>::verify_with_processed_vk(&ark_pvk, &ark_public_inputs, &ark_proof)
            .map_err(|e| format!("Proof verification failed: {e}"))?;

    if !ark_verified {
        println!("❌ Proof verification failed for Sui");
    } else {
        println!("✅ SP1 proof successfully converted and verified for Sui");
    }


    Ok(SuiProofData {
        vkey_bytes: ark_groth16_serialized,
        public_inputs_bytes: ark_public_inputs_serialized,
        proof_bytes: ark_proof_serialized,
    })
}
