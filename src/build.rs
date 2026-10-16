use crate::bundle::{Bundle, Entry};
use crate::lines::{field, split_lines};
use crate::spec::Spec;

/// Counts the values accepted and skipped by one sketch during a build.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stat {
    pub accepted: u64,
    pub skipped: u64,
}

/// Builds each requested sketch from the input files in order.
pub fn build(specs: &[Spec], delim: &[u8], inputs: &[Vec<u8>]) -> (Bundle, Vec<Stat>) {
    let mut entries: Vec<Entry> = specs
        .iter()
        .map(|spec| Entry {
            name: spec.name.clone(),
            sketch: spec.new_sketch(),
        })
        .collect();
    let mut stats = vec![Stat::default(); specs.len()];

    for (index, spec) in specs.iter().enumerate() {
        for input in inputs {
            for line in split_lines(input) {
                match field(line, delim, spec.field).filter(|value| !value.is_empty()) {
                    Some(value) if entries[index].sketch.add(value) => {
                        stats[index].accepted = stats[index].accepted.saturating_add(1);
                    }
                    _ => stats[index].skipped = stats[index].skipped.saturating_add(1),
                }
            }
        }
    }

    (Bundle { entries }, stats)
}
