//! Seeded random data sets and the guest input built from them.

use rand::{Rng, RngExt, SeedableRng, rngs::ChaCha20Rng};
use sp1_sdk::SP1Stdin;

use crate::{
    native::{self, Lanes},
    variant::{NODE_SIZE, Variant},
};

/// The KoalaBear prime, the field of SP1's Poseidon2 precompile.
pub const KOALA_BEAR_PRIME: u32 = 2_130_706_433;

/// The public values every guest commits: `count || size || acc`, little-endian.
pub type PublicOutput = [u8; 48];

/// `count` random data sets of `size` bytes. Each set starts on an 8-byte boundary, because the
/// guests read words from them and the guest target traps on misaligned word loads.
pub struct Dataset {
    pub size: usize,
    pub count: usize,
    pub stride: usize,
    /// Random bytes, zero-padded to the stride.
    pub raw: Vec<u8>,
    /// Canonical KoalaBear elements as 16 little-endian `u32`s per set. Only at the node size.
    pub elements: Vec<u8>,
    /// `raw` with each 32-bit word reversed. Only at the node size. The `sha256_node` guest reads
    /// `raw` as native words, which are the big-endian words of this message.
    pub swapped: Vec<u8>,
}

impl Dataset {
    /// Generates the data for one size and count. The same inputs always give the same data, in
    /// whatever order a sweep visits them.
    pub fn generate(seed: u64, size: usize, count: usize) -> Self {
        let mut rng_seed = [0u8; 32];
        rng_seed[..8].copy_from_slice(&seed.to_le_bytes());
        rng_seed[8..16].copy_from_slice(&(size as u64).to_le_bytes());
        rng_seed[16..24].copy_from_slice(&(count as u64).to_le_bytes());
        let mut rng = ChaCha20Rng::from_seed(rng_seed);

        let stride = size.next_multiple_of(8);
        let mut raw = vec![0u8; count * stride];
        for set in raw.chunks_exact_mut(stride) {
            rng.fill_bytes(&mut set[..size]);
        }

        let (elements, swapped) = if size == NODE_SIZE {
            let mut elements = Vec::with_capacity(count * NODE_SIZE);
            for _ in 0..count * 16 {
                let element: u32 = rng.random_range(0..KOALA_BEAR_PRIME);
                elements.extend_from_slice(&element.to_le_bytes());
            }
            let swapped = raw
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|word| [word[3], word[2], word[1], word[0]])
                .collect();
            (elements, swapped)
        } else {
            (Vec::new(), Vec::new())
        };

        Self {
            size,
            count,
            stride,
            raw,
            elements,
            swapped,
        }
    }

    /// Yields the bytes of each set, without padding.
    pub fn sets(&self) -> impl Iterator<Item = &[u8]> {
        self.raw
            .chunks_exact(self.stride)
            .map(|set| &set[..self.size])
    }

    /// Builds the guest input. Size and count are small; the data goes in unframed so the guest
    /// can use it in place.
    pub fn stdin(&self, variant: Variant) -> SP1Stdin {
        let mut stdin = SP1Stdin::new();
        stdin.write(&(self.size as u64));
        stdin.write(&(self.count as u64));
        stdin.write_slice(if variant.takes_field_elements() {
            &self.elements
        } else {
            &self.raw
        });
        stdin
    }

    /// The public values a guest must commit, computed natively.
    pub fn expected_output(&self, variant: Variant) -> PublicOutput {
        public_output(self.count, self.size, &native::accumulate(variant, self))
    }
}

/// Encodes public values the way the guests commit them.
pub fn public_output(count: usize, size: usize, lanes: &Lanes) -> PublicOutput {
    let mut output = [0u8; 48];
    output[..8].copy_from_slice(&(count as u64).to_le_bytes());
    output[8..16].copy_from_slice(&(size as u64).to_le_bytes());
    for (bytes, lane) in output[16..].as_chunks_mut::<8>().0.iter_mut().zip(lanes) {
        *bytes = lane.to_le_bytes();
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_gives_the_same_data() {
        let first = Dataset::generate(1, 64, 5);
        let second = Dataset::generate(1, 64, 5);
        assert_eq!(first.raw, second.raw);
        assert_eq!(first.elements, second.elements);
        assert_ne!(first.raw, Dataset::generate(2, 64, 5).raw);
        // Each point has its own stream, so a sweep's order cannot change the data.
        assert_ne!(first.raw[..64], Dataset::generate(1, 64, 6).raw[..64]);
    }

    #[test]
    fn sets_sit_on_8_byte_strides_with_zero_padding() {
        for size in [1, 7, 8, 9, 40, 63, 64, 65] {
            let data = Dataset::generate(3, size, 4);
            assert_eq!(data.stride % 8, 0);
            assert_eq!(data.raw.len(), 4 * data.stride);
            assert_eq!(data.sets().count(), 4);
            for set in data.raw.chunks_exact(data.stride) {
                assert!(set[size..].iter().all(|byte| *byte == 0));
            }
            assert!(data.sets().all(|set| set.len() == size));
        }
    }

    #[test]
    fn elements_are_canonical() {
        let data = Dataset::generate(4, 64, 100);
        assert_eq!(data.elements.len(), 100 * 64);
        for bytes in data.elements.as_chunks::<4>().0 {
            assert!(u32::from_le_bytes(*bytes) < KOALA_BEAR_PRIME);
        }
    }

    #[test]
    fn node_buffers_exist_only_at_the_node_size() {
        assert!(Dataset::generate(5, 96, 2).elements.is_empty());
        assert!(Dataset::generate(5, 96, 2).swapped.is_empty());
        assert_eq!(Dataset::generate(5, 64, 2).swapped.len(), 128);
    }

    #[test]
    fn public_output_is_little_endian() {
        let output = public_output(2, 64, &[3, 4, 5, 6]);
        assert_eq!(output[0], 2);
        assert_eq!(output[8], 64);
        assert_eq!(output[16], 3);
        assert_eq!(output[24], 4);
        assert_eq!(output[32], 5);
        assert_eq!(output[40], 6);
    }
}
