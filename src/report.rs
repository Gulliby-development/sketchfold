use crate::bundle::Bundle;
use crate::frequent::Frequent;
use crate::sketch::Sketch;
use crate::text::{escape_value, plural};

/// Renders one summary line per sketch, preserving bundle order.
pub fn show(bundle: &Bundle) -> String {
    let name_width = bundle
        .entries
        .iter()
        .map(|entry| entry.name.len())
        .max()
        .unwrap_or(0);
    let mut output = String::new();

    for entry in &bundle.entries {
        let parameter = entry.sketch.param_text();
        let detail = match &entry.sketch {
            Sketch::Distinct(sketch) => {
                let error = sketch.error_tenths();
                format!(
                    "about {} distinct, error {}.{}%",
                    sketch.estimate(),
                    error / 10,
                    error % 10
                )
            }
            Sketch::Top(sketch) => format!(
                "{}, slack {}",
                plural(sketch.items.len() as u64, "counter", "counters"),
                sketch.slack()
            ),
            Sketch::Hist(sketch) if sketch.n == 0 => "empty".to_owned(),
            Sketch::Hist(sketch) => format!(
                "min {}, max {}, p50 {}, p90 {}, p99 {}",
                sketch.min,
                sketch.max,
                sketch.quantile(50_000).unwrap_or(sketch.min),
                sketch.quantile(90_000).unwrap_or(sketch.max),
                sketch.quantile(99_000).unwrap_or(sketch.max)
            ),
            Sketch::Member(sketch) => {
                let set = sketch.bits.iter().filter(|&&bit| bit).count();
                format!("{set} of {} bits set", sketch.bits.len())
            }
        };
        output.push_str(&format!(
            "{:<name_width$}  {:<8}  {}  n={}  {}\n",
            entry.name,
            entry.sketch.kind(),
            parameter,
            entry.sketch.n(),
            detail
        ));
    }

    output
}

/// Renders a frequent-values report, limiting the displayed counters if asked.
pub fn top(name: &str, frequent: &Frequent, limit: Option<usize>) -> String {
    let width = frequent
        .items
        .iter()
        .map(|(_, count)| count.to_string().len())
        .max()
        .unwrap_or(1);
    let mut output = format!("{name}: n={}, slack {}\n", frequent.n, frequent.slack());

    for (value, count) in frequent.items.iter().take(limit.unwrap_or(usize::MAX)) {
        output.push_str(&format!("{count:>width$}  {}\n", escape_value(value)));
    }

    output
}

/// Parses a canonical percentage into thousandths of a percent.
pub fn parse_q(text: &str) -> Option<u32> {
    let (whole_text, fraction_text) = match text.split_once('.') {
        Some((whole, fraction)) if !fraction.contains('.') => (whole, Some(fraction)),
        Some(_) => return None,
        None => (text, None),
    };
    let whole_bytes = whole_text.as_bytes();
    if whole_bytes.is_empty()
        || whole_bytes.len() > 3
        || !whole_bytes.iter().all(u8::is_ascii_digit)
        || (whole_bytes.len() > 1 && whole_bytes[0] == b'0')
    {
        return None;
    }

    let mut whole = 0_u32;
    for &digit in whole_bytes {
        whole = whole * 10 + u32::from(digit - b'0');
    }

    let fraction = match fraction_text {
        Some(text) if (1..=3).contains(&text.len()) => {
            if !text.as_bytes().iter().all(u8::is_ascii_digit) {
                return None;
            }
            let mut value = 0_u32;
            for &digit in text.as_bytes() {
                value = value * 10 + u32::from(digit - b'0');
            }
            value
                * match text.len() {
                    1 => 100,
                    2 => 10,
                    _ => 1,
                }
        }
        Some(_) => return None,
        None => 0,
    };

    if whole > 100 || (whole == 100 && fraction != 0) {
        return None;
    }
    Some(whole * 1000 + fraction)
}
