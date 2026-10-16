use crate::hash::{fnv64, hash64};

/// A Bloom-filter membership sketch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub b: u32,
    pub n: u64,
    pub bits: Vec<bool>,
}

impl Member {
    /// Creates a sketch with `2^b` bits.
    ///
    /// The caller is responsible for supplying a supported size (6..=20).
    pub fn new(b: u32) -> Self {
        Self {
            b,
            n: 0,
            bits: vec![false; 1_usize << b],
        }
    }

    /// Adds a byte-oriented value to the filter.
    pub fn add(&mut self, value: &[u8]) {
        for index in self.positions(value) {
            self.bits[index] = true;
        }
        self.n = self.n.saturating_add(1);
    }

    /// Returns whether all four bit positions for the value are set.
    pub fn has(&self, value: &[u8]) -> bool {
        self.positions(value)
            .into_iter()
            .all(|index| self.bits[index])
    }

    /// Unions another same-sized filter into this one.
    pub fn merge(&mut self, other: &Self) {
        for (bit, other_bit) in self.bits.iter_mut().zip(&other.bits) {
            *bit |= other_bit;
        }
        self.n = self.n.saturating_add(other.n);
    }

    fn positions(&self, value: &[u8]) -> [usize; 4] {
        let first = hash64(value);
        let step = fnv64(value) | 1;
        let mask = (self.bits.len() - 1) as u64;

        std::array::from_fn(|i| (first.wrapping_add((i as u64).wrapping_mul(step)) & mask) as usize)
    }
}
