/// An integer histogram with deterministic logarithmic buckets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hist {
    pub s: u32,
    pub n: u64,
    pub min: u64,
    pub max: u64,
    pub buckets: Vec<(u32, u64, u64)>,
}

impl Hist {
    /// Creates a histogram with precision `s`.
    ///
    /// The caller is responsible for supplying a supported precision (1..=8).
    pub fn new(s: u32) -> Self {
        Self {
            s,
            n: 0,
            min: 0,
            max: 0,
            buckets: Vec::new(),
        }
    }

    /// Adds an integer value to the histogram.
    pub fn add(&mut self, value: u64) {
        let bit_length = u64::BITS - value.leading_zeros();
        let shift = bit_length.saturating_sub(self.s + 1);
        let top = value >> shift;

        match self
            .buckets
            .binary_search_by_key(&(shift, top), |(bucket_shift, bucket_top, _)| {
                (*bucket_shift, *bucket_top)
            }) {
            Ok(index) => self.buckets[index].2 = self.buckets[index].2.saturating_add(1),
            Err(index) => self.buckets.insert(index, (shift, top, 1)),
        }

        if self.n == 0 {
            self.min = value;
            self.max = value;
        } else {
            self.min = self.min.min(value);
            self.max = self.max.max(value);
        }
        self.n = self.n.saturating_add(1);
    }

    /// Merges another histogram with the same precision into this one.
    pub fn merge(&mut self, other: &Self) {
        for &(shift, top, count) in &other.buckets {
            match self
                .buckets
                .binary_search_by_key(&(shift, top), |(bucket_shift, bucket_top, _)| {
                    (*bucket_shift, *bucket_top)
                }) {
                Ok(index) => {
                    self.buckets[index].2 = self.buckets[index].2.saturating_add(count);
                }
                Err(index) => self.buckets.insert(index, (shift, top, count)),
            }
        }

        if other.n > 0 {
            if self.n == 0 {
                self.min = other.min;
                self.max = other.max;
            } else {
                self.min = self.min.min(other.min);
                self.max = self.max.max(other.max);
            }
        }
        self.n = self.n.saturating_add(other.n);
    }

    /// Returns the approximate value at quantile `q` in thousandths of a percent.
    pub fn quantile(&self, q: u32) -> Option<u64> {
        if self.n == 0 {
            return None;
        }

        let numerator = u128::from(self.n) * u128::from(q);
        let denominator = 100_000_u128;
        let rank = numerator.div_ceil(denominator).max(1);

        let mut cumulative = 0_u128;
        for &(shift, top, count) in &self.buckets {
            cumulative += u128::from(count);
            if cumulative >= rank {
                let upper = ((u128::from(top) + 1) << shift) - 1;
                return Some(upper.clamp(u128::from(self.min), u128::from(self.max)) as u64);
            }
        }

        None
    }
}
