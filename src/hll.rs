use crate::hash::hash64;

/// A HyperLogLog distinct-count sketch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hll {
    pub p: u32,
    pub n: u64,
    pub regs: Vec<u8>,
}

impl Hll {
    /// Creates a sketch with precision `p`.
    ///
    /// The caller is responsible for supplying a supported precision (4..=16).
    pub fn new(p: u32) -> Self {
        Self {
            p,
            n: 0,
            regs: vec![0; 1_usize << p],
        }
    }

    /// Adds a byte-oriented value to the sketch.
    pub fn add(&mut self, value: &[u8]) {
        self.n = self.n.saturating_add(1);

        let hash = hash64(value);
        let index = (hash >> (64 - self.p)) as usize;
        let suffix = hash << self.p;
        let rank = suffix.leading_zeros().min(64 - self.p) + 1;
        self.regs[index] = self.regs[index].max(rank as u8);
    }

    /// Merges another sketch with the same precision into this one.
    pub fn merge(&mut self, other: &Self) {
        for (register, other_register) in self.regs.iter_mut().zip(&other.regs) {
            *register = (*register).max(*other_register);
        }
        self.n = self.n.saturating_add(other.n);
    }

    /// Returns the rounded HyperLogLog cardinality estimate.
    pub fn estimate(&self) -> u64 {
        let m = self.regs.len() as f64;
        let alpha = match self.regs.len() {
            16 => 0.673,
            32 => 0.697,
            64 => 0.709,
            _ => 0.7213 / (1.0 + 1.079 / m),
        };

        let mut sum = 0.0;
        let mut zeros = 0_u64;
        for &register in &self.regs {
            sum += 1.0 / 2.0_f64.powi(i32::from(register));
            if register == 0 {
                zeros += 1;
            }
        }

        let raw = alpha * m * m / sum;
        let estimate = if raw <= 2.5 * m && zeros > 0 {
            m * (m / zeros as f64).ln()
        } else {
            raw
        };

        (estimate + 0.5).floor() as u64
    }

    /// Returns the estimated relative error in tenths of a percent.
    pub fn error_tenths(&self) -> u64 {
        let m = self.regs.len() as f64;
        (104.0 / m.sqrt() * 10.0 + 0.5).floor() as u64
    }
}
