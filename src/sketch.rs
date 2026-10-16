use crate::frequent::Frequent;
use crate::hist::Hist;
use crate::hll::Hll;
use crate::member::Member;
use crate::text::parse_u64;

/// A common interface for the four byte-oriented sketches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sketch {
    Distinct(Hll),
    Top(Frequent),
    Hist(Hist),
    Member(Member),
}

impl Sketch {
    /// Returns the lowercase kind name.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Distinct(_) => "distinct",
            Self::Top(_) => "top",
            Self::Hist(_) => "hist",
            Self::Member(_) => "member",
        }
    }

    /// Returns the sketch's canonical parameter text.
    pub fn param_text(&self) -> String {
        match self {
            Self::Distinct(sketch) => format!("p={}", sketch.p),
            Self::Top(sketch) => format!("k={}", sketch.k),
            Self::Hist(sketch) => format!("s={}", sketch.s),
            Self::Member(sketch) => format!("b={}", sketch.b),
        }
    }

    /// Returns the wrapped sketch's accepted value count.
    pub fn n(&self) -> u64 {
        match self {
            Self::Distinct(sketch) => sketch.n,
            Self::Top(sketch) => sketch.n,
            Self::Hist(sketch) => sketch.n,
            Self::Member(sketch) => sketch.n,
        }
    }

    /// Adds a byte-oriented value to this sketch.
    ///
    /// Histograms accept only canonical unsigned decimal integers.
    pub fn add(&mut self, value: &[u8]) -> bool {
        match self {
            Self::Distinct(sketch) => {
                sketch.add(value);
                true
            }
            Self::Top(sketch) => {
                sketch.add(value);
                true
            }
            Self::Hist(sketch) => {
                let Some(value) = parse_u64(value) else {
                    return false;
                };
                sketch.add(value);
                true
            }
            Self::Member(sketch) => {
                sketch.add(value);
                true
            }
        }
    }

    /// Merges another compatible sketch into this one.
    ///
    /// Incompatible kinds and parameters are rejected before either sketch is
    /// changed.
    pub fn merge(&mut self, other: &Self) -> Result<(), String> {
        if self.kind() != other.kind() {
            return Err(format!("cannot merge {} and {}", self.kind(), other.kind()));
        }

        match (self, other) {
            (Self::Distinct(left), Self::Distinct(right)) => {
                if left.p != right.p {
                    return Err(format!("precision differs ({} vs {})", left.p, right.p));
                }
                left.merge(right);
            }
            (Self::Top(left), Self::Top(right)) => {
                if left.k != right.k {
                    return Err(format!("capacity differs ({} vs {})", left.k, right.k));
                }
                left.merge(right);
            }
            (Self::Hist(left), Self::Hist(right)) => {
                if left.s != right.s {
                    return Err(format!("bits differs ({} vs {})", left.s, right.s));
                }
                left.merge(right);
            }
            (Self::Member(left), Self::Member(right)) => {
                if left.b != right.b {
                    return Err(format!("size differs ({} vs {})", left.b, right.b));
                }
                left.merge(right);
            }
            _ => unreachable!("kind names were checked before matching variants"),
        }

        Ok(())
    }
}
