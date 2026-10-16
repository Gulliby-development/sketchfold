use crate::error::CliError;
use crate::sketch::Sketch;

/// A named sketch in a bundle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub sketch: Sketch,
}

/// An ordered collection of named sketches.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bundle {
    pub entries: Vec<Entry>,
}

impl Bundle {
    /// Returns the sketch associated with `name`, if present.
    pub fn get(&self, name: &str) -> Option<&Sketch> {
        self.entries
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| &entry.sketch)
    }
}

/// Merges bundles in order, retaining each name's first position.
pub fn merge_all(bundles: Vec<Bundle>) -> Result<Bundle, CliError> {
    let mut merged = Bundle::default();

    for bundle in bundles {
        for entry in bundle.entries {
            if let Some(existing) = merged
                .entries
                .iter_mut()
                .find(|existing| existing.name == entry.name)
            {
                existing
                    .sketch
                    .merge(&entry.sketch)
                    .map_err(|message| CliError(format!("{}: {message}", entry.name)))?;
            } else {
                merged.entries.push(entry);
            }
        }
    }

    Ok(merged)
}
