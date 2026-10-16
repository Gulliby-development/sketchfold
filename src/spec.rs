use crate::frequent::Frequent;
use crate::hist::Hist;
use crate::hll::Hll;
use crate::member::Member;
use crate::sketch::Sketch;
use crate::text::parse_count;

/// The sketch algorithm selected by a build specification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Distinct,
    Top,
    Hist,
    Member,
}

/// A parsed sketch name, algorithm, input field and parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub name: String,
    pub kind: Kind,
    pub field: usize,
    pub param: u32,
}

impl Spec {
    /// Constructs an empty sketch matching this specification.
    pub fn new_sketch(&self) -> Sketch {
        match self.kind {
            Kind::Distinct => Sketch::Distinct(Hll::new(self.param)),
            Kind::Top => Sketch::Top(Frequent::new(self.param as usize)),
            Kind::Hist => Sketch::Hist(Hist::new(self.param)),
            Kind::Member => Sketch::Member(Member::new(self.param)),
        }
    }
}

/// Parses a specification in the form NAME:KIND:FIELD[:PARAM].
pub fn parse_spec(text: &str) -> Option<Spec> {
    let parts: Vec<&str> = text.split(':').collect();
    if !(parts.len() == 3 || parts.len() == 4) || !valid_name(parts[0]) {
        return None;
    }

    let kind = match parts[1] {
        "distinct" => Kind::Distinct,
        "top" => Kind::Top,
        "hist" => Kind::Hist,
        "member" => Kind::Member,
        _ => return None,
    };

    let field = parse_count(parts[2], 0, 99)? as usize;
    let (minimum, maximum, default) = parameter_range(kind);
    let param = if parts.len() == 4 {
        parse_count(parts[3], u64::from(minimum), u64::from(maximum))? as u32
    } else {
        default
    };

    Some(Spec {
        name: parts[0].to_owned(),
        kind,
        field,
        param,
    })
}

/// Checks the shared bundle/spec name grammar.
pub(crate) fn valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.len() > 32 || !bytes[0].is_ascii_lowercase() {
        return false;
    }

    bytes[1..].iter().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_' || *byte == b'-'
    })
}

fn parameter_range(kind: Kind) -> (u32, u32, u32) {
    match kind {
        Kind::Distinct => (4, 16, 12),
        Kind::Top => (1, 1000, 16),
        Kind::Hist => (1, 8, 4),
        Kind::Member => (6, 20, 14),
    }
}
