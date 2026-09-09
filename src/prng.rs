use rand::{Rng, RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use static_assertions::{assert_eq_align, assert_eq_size};
use thiserror::Error;

#[derive(Debug, PartialEq, Error)]
pub enum PRNGError {
    #[error("Invalid PRNG state descriptor")]
    InvalidState,
}

#[repr(C, align(32))]
#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PrngStatePOD {
    pub seed: u64,      // Offset 0..8
    pub stream_id: u64, // Offset 8..16
    pub word_pos: u128, // Offset 16..32
}

impl PrngStatePOD {
    pub const fn new(seed: u64, stream_id: u64, word_pos: u128) -> Self {
        Self {
            seed,
            stream_id,
            word_pos,
        }
    }
}

#[repr(C, align(32))]
struct Align32Marker;

assert_eq_size!(PrngStatePOD, [u8; 32]);
assert_eq_align!(PrngStatePOD, Align32Marker);

pub struct DeterministicRng {
    inner: ChaCha8Rng,
    seed: u64,
}

impl DeterministicRng {
    pub fn new(seed: u64) -> Self {
        Self {
            inner: ChaCha8Rng::seed_from_u64(seed),
            seed,
        }
    }

    pub fn gen_range_f32(&mut self, low: f32, high: f32) -> f32 {
        let mantissa = self.inner.next_u32() >> 9;
        let unit = f32::from_bits(0x3F80_0000 | mantissa) - 1.0;
        low + unit * (high - low)
    }

    pub fn gen_bool(&mut self, probability: f64) -> bool {
        self.inner.gen_bool(probability)
    }

    pub fn fork(&mut self) -> Self {
        let child_seed = self.inner.next_u64();
        let stream = self.inner.get_stream();
        let child = Self::new(child_seed);
        child.with_stream(stream.wrapping_add(1))
    }

    pub fn export_pod(&self) -> PrngStatePOD {
        PrngStatePOD::new(self.seed, self.inner.get_stream(), self.inner.get_word_pos())
    }

    pub fn from_pod(pod: PrngStatePOD) -> Result<Self, PRNGError> {
        let mut inner = ChaCha8Rng::seed_from_u64(pod.seed);
        inner.set_stream(pod.stream_id);
        inner.set_word_pos(pod.word_pos);
        Ok(Self {
            inner,
            seed: pod.seed,
        })
    }

    fn with_stream(mut self, stream_id: u64) -> Self {
        self.inner.set_stream(stream_id);
        self
    }
}

impl Serialize for DeterministicRng {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.export_pod().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DeterministicRng {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let pod = PrngStatePOD::deserialize(deserializer)?;
        DeterministicRng::from_pod(pod).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_sequence(rng: &mut DeterministicRng, n: usize) -> Vec<u32> {
        (0..n).map(|_| rng.inner.next_u32()).collect()
    }

    #[test]
    fn test_identical_sequence() {
        let mut a = DeterministicRng::new(0x5EED_CAFE_BABE_1234);
        let mut b = DeterministicRng::new(0x5EED_CAFE_BABE_1234);
        assert_eq!(sample_sequence(&mut a, 16), sample_sequence(&mut b, 16));

        let mut af = DeterministicRng::new(0xDEAD_BEEF_0001);
        let mut bf = DeterministicRng::new(0xDEAD_BEEF_0001);
        for _ in 0..100 {
            let (va, vb) = (af.gen_range_f32(-10.0, 10.0), bf.gen_range_f32(-10.0, 10.0));
            assert_eq!(va.to_bits(), vb.to_bits());
            assert!(va >= -10.0 && va < 10.0);
        }
    }

    #[test]
    fn test_serde_restoration() {
        let mut original = DeterministicRng::new(0xFEED_F00D_CAFE_1);
        let mut expected = original.fork();
        for _ in 0..10 {
            let _ = expected.gen_range_f32(0.0, 1.0);
        }

        let json = serde_json::to_string(&original).expect("serialize ok");
        let restored: DeterministicRng = serde_json::from_str(&json).expect("deserialize ok");

        let pod = restored.export_pod();
        assert_eq!(pod.seed, original.export_pod().seed);
        assert_eq!(pod.stream_id, original.export_pod().stream_id);
        assert_eq!(pod.word_pos, original.export_pod().word_pos);

        let mut original_mut = original;
        let mut restored_mut = restored;
        for _ in 0..100 {
            assert_eq!(
                original_mut.gen_range_f32(0.0, 100.0).to_bits(),
                restored_mut.gen_range_f32(0.0, 100.0).to_bits()
            );
        }

        let exp = expected.export_pod();
        assert_eq!(pod.stream_id, exp.stream_id - 1);
    }

    #[test]
    fn test_fork_stream_hierarchy() {
        let mut parent = DeterministicRng::new(0xABCDEF_1234567);
        let parent_stream = parent.inner.get_stream();
        let child = parent.fork();
        assert_eq!(child.inner.get_stream(), parent_stream.wrapping_add(1));
        assert_ne!(child.inner.get_word_pos(), parent.inner.get_word_pos());
    }

    #[test]
    fn test_state_bytes_zero_alloc() {
        let mut rng = DeterministicRng::new(0xCAFEBABE_1);
        let pod = alloc_counter::deny_alloc(|| rng.export_pod());
        assert_eq!(pod.seed, 0xCAFEBABE_1);

        let v = alloc_counter::deny_alloc(|| rng.gen_range_f32(0.0, 1.0));
        assert!(v >= 0.0 && v < 1.0);

        let b = alloc_counter::deny_alloc(|| rng.gen_bool(0.5));
        let _ = b;
    }
}