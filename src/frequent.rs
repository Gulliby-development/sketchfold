/// A bounded Misra-Gries frequent-value sketch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frequent {
    pub k: usize,
    pub n: u64,
    pub items: Vec<(Vec<u8>, u64)>,
}

impl Frequent {
    /// Creates a sketch with capacity `k`.
    ///
    /// The caller is responsible for supplying a supported capacity (1..=1000).
    pub fn new(k: usize) -> Self {
        Self {
            k,
            n: 0,
            items: Vec::new(),
        }
    }

    /// Adds a byte-oriented value to the sketch.
    pub fn add(&mut self, value: &[u8]) {
        self.n = self.n.saturating_add(1);

        if let Some((_, count)) = self
            .items
            .iter_mut()
            .find(|(item, _)| item.as_slice() == value)
        {
            *count = count.saturating_add(1);
        } else if self.items.len() < self.k {
            self.items.push((value.to_vec(), 1));
        } else {
            for (_, count) in &mut self.items {
                *count -= 1;
            }
            self.items.retain(|(_, count)| *count > 0);
        }

        self.sort_items();
    }

    /// Merges another sketch with the same capacity into this one.
    pub fn merge(&mut self, other: &Self) {
        let mut combined: Vec<(Vec<u8>, u128)> = self
            .items
            .iter()
            .chain(&other.items)
            .map(|(value, count)| (value.clone(), u128::from(*count)))
            .collect();

        combined.sort_by(|left, right| left.0.cmp(&right.0));
        let mut merged: Vec<(Vec<u8>, u128)> = Vec::with_capacity(combined.len());
        for (value, count) in combined {
            if let Some((last_value, last_count)) = merged.last_mut()
                && *last_value == value
            {
                *last_count += count;
                continue;
            }
            merged.push((value, count));
        }

        merged.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        if merged.len() > self.k {
            let threshold = merged[self.k].1;
            merged.retain_mut(|(_, count)| {
                *count = count.saturating_sub(threshold);
                *count > 0
            });
        }

        self.items = merged
            .into_iter()
            .map(|(value, count)| (value, count.min(u128::from(u64::MAX)) as u64))
            .collect();
        self.n = self.n.saturating_add(other.n);
        self.sort_items();
    }

    /// Returns the guaranteed additive slack of the stored counters.
    pub fn slack(&self) -> u64 {
        let total = self
            .items
            .iter()
            .map(|(_, count)| u128::from(*count))
            .sum::<u128>();
        let deficit = u128::from(self.n).saturating_sub(total);
        (deficit / (self.k as u128 + 1)).min(u128::from(u64::MAX)) as u64
    }

    fn sort_items(&mut self) {
        self.items
            .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    }
}
