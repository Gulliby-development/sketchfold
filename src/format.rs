use crate::bundle::{Bundle, Entry};
use crate::error::FormatError;
use crate::hash::checksum32;
use crate::hist::Hist;
use crate::hll::Hll;
use crate::member::Member;
use crate::sketch::Sketch;
use crate::spec::{Kind, valid_name};
use crate::text::{hex_decode, hex_encode, parse_u64};

/// Encodes a bundle in its canonical ASCII, LF-terminated form.
pub fn encode(bundle: &Bundle) -> String {
    let mut text = String::from("sketchfold 1\n");

    for entry in &bundle.entries {
        match &entry.sketch {
            Sketch::Distinct(sketch) => {
                text.push_str(&format!(
                    "sketch {} distinct p={} n={}\n",
                    entry.name, sketch.p, sketch.n
                ));
                text.push_str("regs ");
                text.push_str(&hex_encode(&sketch.regs));
                text.push('\n');
            }
            Sketch::Top(sketch) => {
                text.push_str(&format!(
                    "sketch {} top k={} n={}\n",
                    entry.name, sketch.k, sketch.n
                ));
                let mut items: Vec<_> = sketch.items.iter().collect();
                items
                    .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
                for (value, count) in items {
                    text.push_str(&format!("item {count} {}\n", hex_encode(value)));
                }
            }
            Sketch::Hist(sketch) => {
                text.push_str(&format!(
                    "sketch {} hist s={} n={} min={} max={}\n",
                    entry.name, sketch.s, sketch.n, sketch.min, sketch.max
                ));
                let mut buckets = sketch.buckets.clone();
                buckets.sort_by_key(|(shift, top, _)| (*shift, *top));
                for (shift, top, count) in buckets {
                    text.push_str(&format!("bucket {shift} {top} {count}\n"));
                }
            }
            Sketch::Member(sketch) => {
                text.push_str(&format!(
                    "sketch {} member b={} n={}\n",
                    entry.name, sketch.b, sketch.n
                ));
                text.push_str("bits ");
                text.push_str(&encode_bits(&sketch.bits));
                text.push('\n');
            }
        }
    }

    let checksum = checksum32(text.as_bytes());
    text.push_str(&format!("end {checksum}\n"));
    text
}

/// Decodes and validates a physical-line bundle without normalizing its bytes.
pub fn decode(bytes: &[u8]) -> Result<Bundle, FormatError> {
    let lines = physical_lines(bytes);
    if lines
        .first()
        .is_none_or(|line| line.bytes != b"sketchfold 1")
    {
        return Err(format_error(1, "not a sketchfold bundle"));
    }

    let mut entries = Vec::new();
    let mut current: Option<Current> = None;

    for (index, line) in lines.iter().enumerate().skip(1) {
        if (is_boundary_word(line.bytes, b"sketch") || is_boundary_word(line.bytes, b"end"))
            && let Some(sketch) = current.take()
        {
            finalize(&sketch, line.number)?;
            entries.push(sketch.entry);
        }

        if is_boundary_word(line.bytes, b"sketch") {
            let (sketch, name) = parse_sketch_header(line.bytes, line.number)
                .ok_or_else(|| format_error(line.number, "bad sketch line"))?;
            if entries.iter().any(|entry: &Entry| entry.name == name) {
                return Err(format_error(
                    line.number,
                    &format!("duplicate sketch name \"{name}\""),
                ));
            }
            current = Some(sketch);
            continue;
        }

        if is_boundary_word(line.bytes, b"end") {
            if !line.bytes.starts_with(b"end ") {
                return Err(format_error(line.number, "unexpected line"));
            }
            if !valid_end_line(line) {
                return Err(format_error(line.number, "bad end line"));
            }
            let expected = checksum32(&bytes[..line.start]);
            if &line.bytes[4..] != expected.as_bytes() {
                return Err(format_error(line.number, "checksum mismatch"));
            }
            if let Some(trailing) = lines.get(index + 1) {
                return Err(format_error(trailing.number, "unexpected data after end"));
            }
            return Ok(Bundle { entries });
        }

        let Some(sketch) = current.as_mut() else {
            return Err(format_error(line.number, "unexpected line"));
        };
        parse_payload(sketch, line.bytes, line.number)?;
    }

    if let Some(sketch) = current {
        finalize(&sketch, lines.len() + 1)?;
        entries.push(sketch.entry);
    }
    Err(format_error(lines.len() + 1, "missing end line"))
}

struct PhysicalLine<'a> {
    bytes: &'a [u8],
    start: usize,
    number: usize,
    terminated: bool,
}

struct Current {
    entry: Entry,
    header_line: usize,
    has_payload: bool,
    total: Option<u128>,
}

fn physical_lines(bytes: &[u8]) -> Vec<PhysicalLine<'_>> {
    let mut lines = Vec::new();
    let mut start = 0;

    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            lines.push(PhysicalLine {
                bytes: &bytes[start..index],
                start,
                number: lines.len() + 1,
                terminated: true,
            });
            start = index + 1;
        }
    }

    if start < bytes.len() {
        lines.push(PhysicalLine {
            bytes: &bytes[start..],
            start,
            number: lines.len() + 1,
            terminated: false,
        });
    }
    lines
}

fn is_boundary_word(line: &[u8], word: &[u8]) -> bool {
    line.split(|byte| *byte == b' ').next() == Some(word)
}

fn parse_sketch_header(line: &[u8], header_line: usize) -> Option<(Current, String)> {
    let parts: Vec<&[u8]> = line.split(|byte| *byte == b' ').collect();
    let name = std::str::from_utf8(parts.get(1)?).ok()?;
    if !valid_name(name) {
        return None;
    }

    let kind_text = *parts.get(2)?;
    let (kind, parameter, minimum, maximum) = match kind_text {
        b"distinct" => (Kind::Distinct, b"p=".as_slice(), 4, 16),
        b"top" => (Kind::Top, b"k=".as_slice(), 1, 1000),
        b"hist" => (Kind::Hist, b"s=".as_slice(), 1, 8),
        b"member" => (Kind::Member, b"b=".as_slice(), 6, 20),
        _ => return None,
    };

    let (sketch, _header_n) = match kind {
        Kind::Hist => {
            if parts.len() != 7 || parts[0] != b"sketch" {
                return None;
            }
            let param = parse_assignment(parts[3], parameter)?;
            let n = parse_assignment(parts[4], b"n=")?;
            let min = parse_assignment(parts[5], b"min=")?;
            let max = parse_assignment(parts[6], b"max=")?;
            if !(minimum..=maximum).contains(&param) || min > max || (n == 0 && max != 0) {
                return None;
            }
            (
                Sketch::Hist(Hist {
                    s: u32::try_from(param).ok()?,
                    n,
                    min,
                    max,
                    buckets: Vec::new(),
                }),
                n,
            )
        }
        _ => {
            if parts.len() != 5 || parts[0] != b"sketch" {
                return None;
            }
            let param = parse_assignment(parts[3], parameter)?;
            let n = parse_assignment(parts[4], b"n=")?;
            if !(minimum..=maximum).contains(&param) {
                return None;
            }
            let sketch = match kind {
                Kind::Distinct => Sketch::Distinct(Hll {
                    p: u32::try_from(param).ok()?,
                    n,
                    regs: Vec::new(),
                }),
                Kind::Top => Sketch::Top(crate::frequent::Frequent {
                    k: usize::try_from(param).ok()?,
                    n,
                    items: Vec::new(),
                }),
                Kind::Member => Sketch::Member(Member {
                    b: u32::try_from(param).ok()?,
                    n,
                    bits: Vec::new(),
                }),
                Kind::Hist => unreachable!(),
            };
            (sketch, n)
        }
    };

    let sketch = match sketch {
        Sketch::Distinct(mut sketch) => {
            sketch.regs = vec![0; 1_usize << sketch.p];
            Sketch::Distinct(sketch)
        }
        Sketch::Member(mut sketch) => {
            sketch.bits = vec![false; 1_usize << sketch.b];
            Sketch::Member(sketch)
        }
        sketch => sketch,
    };
    Some((
        Current {
            entry: Entry {
                name: name.to_owned(),
                sketch,
            },
            header_line,
            has_payload: false,
            total: Some(0),
        },
        name.to_owned(),
    ))
}

fn parse_assignment(token: &[u8], prefix: &[u8]) -> Option<u64> {
    token.strip_prefix(prefix).and_then(parse_u64)
}

fn parse_payload(current: &mut Current, line: &[u8], number: usize) -> Result<(), FormatError> {
    let parts: Vec<&[u8]> = line.split(|byte| *byte == b' ').collect();
    let word = parts.first().copied().unwrap_or_default();

    match (&mut current.entry.sketch, word) {
        (Sketch::Distinct(sketch), b"regs") => {
            if current.has_payload {
                return Err(format_error(number, "unexpected line"));
            }
            if parts.len() != 2 {
                return Err(format_error(number, "bad regs line"));
            }
            let Some(hex) = std::str::from_utf8(parts[1]).ok().and_then(hex_decode) else {
                return Err(format_error(number, "bad regs line"));
            };
            if hex.len() != (1_usize << sketch.p)
                || hex
                    .iter()
                    .any(|register| u32::from(*register) > 65 - sketch.p)
            {
                return Err(format_error(number, "bad regs line"));
            }
            sketch.regs = hex;
            current.has_payload = true;
            Ok(())
        }
        (Sketch::Member(sketch), b"bits") => {
            if current.has_payload {
                return Err(format_error(number, "unexpected line"));
            }
            if parts.len() != 2 {
                return Err(format_error(number, "bad bits line"));
            }
            let Some(hex) = std::str::from_utf8(parts[1]).ok().and_then(hex_decode) else {
                return Err(format_error(number, "bad bits line"));
            };
            let expected_bits = 1_usize << sketch.b;
            if hex.len() * 2 != expected_bits / 4 {
                return Err(format_error(number, "bad bits line"));
            }
            sketch.bits = decode_bits(&hex, expected_bits);
            current.has_payload = true;
            Ok(())
        }
        (Sketch::Top(sketch), b"item") => {
            if parts.len() != 3 {
                return Err(format_error(number, "bad item line"));
            }
            let Some(count) = parse_u64(parts[1]).filter(|count| *count > 0) else {
                return Err(format_error(number, "bad item line"));
            };
            let Some(value) = std::str::from_utf8(parts[2]).ok().and_then(hex_decode) else {
                return Err(format_error(number, "bad item line"));
            };
            if sketch.items.len() >= sketch.k {
                return Err(format_error(number, "bad item line"));
            }
            if let Some((previous_value, previous_count)) = sketch.items.last()
                && (*previous_count < count
                    || (*previous_count == count && previous_value >= &value))
            {
                return Err(format_error(number, "bad item line"));
            }
            current.total = current
                .total
                .and_then(|total| total.checked_add(u128::from(count)));
            sketch.items.push((value, count));
            Ok(())
        }
        (Sketch::Hist(sketch), b"bucket") => {
            if parts.len() != 4 {
                return Err(format_error(number, "bad bucket line"));
            }
            let (Some(shift), Some(top), Some(count)) = (
                parse_u64(parts[1]),
                parse_u64(parts[2]),
                parse_u64(parts[3]).filter(|count| *count > 0),
            ) else {
                return Err(format_error(number, "bad bucket line"));
            };
            let minimum_top = if shift == 0 { 0 } else { 1_u64 << sketch.s };
            let maximum_top = (1_u64 << (sketch.s + 1)) - 1;
            if shift > u64::from(63 - sketch.s)
                || top < minimum_top
                || top > maximum_top
                || sketch
                    .buckets
                    .last()
                    .is_some_and(|(previous_shift, previous_top, _)| {
                        (*previous_shift, *previous_top) >= (shift as u32, top)
                    })
            {
                return Err(format_error(number, "bad bucket line"));
            }
            current.total = current
                .total
                .and_then(|total| total.checked_add(u128::from(count)));
            sketch.buckets.push((shift as u32, top, count));
            Ok(())
        }
        _ => Err(format_error(number, "unexpected line")),
    }
}

fn finalize(current: &Current, boundary_line: usize) -> Result<(), FormatError> {
    match &current.entry.sketch {
        Sketch::Distinct(_) if !current.has_payload => {
            Err(format_error(boundary_line, "bad regs line"))
        }
        Sketch::Member(_) if !current.has_payload => {
            Err(format_error(boundary_line, "bad bits line"))
        }
        Sketch::Top(sketch)
            if current
                .total
                .is_none_or(|total| total > u128::from(sketch.n)) =>
        {
            Err(format_error(current.header_line, "counts exceed n"))
        }
        Sketch::Hist(sketch)
            if current
                .total
                .is_none_or(|total| total != u128::from(sketch.n)) =>
        {
            Err(format_error(
                current.header_line,
                "bucket counts do not add up to n",
            ))
        }
        _ => Ok(()),
    }
}

fn valid_end_line(line: &PhysicalLine<'_>) -> bool {
    line.terminated
        && line.bytes.len() == 12
        && line.bytes.starts_with(b"end ")
        && line.bytes[4..].iter().all(|byte| is_lower_hex(*byte))
}

fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

fn encode_bits(bits: &[bool]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bits.len() / 4);
    for group in bits.chunks(4) {
        let mut nibble = 0_usize;
        for (index, bit) in group.iter().enumerate() {
            if *bit {
                nibble |= 1 << index;
            }
        }
        encoded.push(char::from(HEX[nibble]));
    }
    encoded
}

fn decode_bits(bytes: &[u8], bit_count: usize) -> Vec<bool> {
    let mut bits = Vec::with_capacity(bit_count);
    for byte in bytes {
        for nibble in [byte >> 4, byte & 0x0f] {
            for bit in 0..4 {
                bits.push(nibble & (1 << bit) != 0);
            }
        }
    }
    bits
}

fn format_error(line: usize, message: &str) -> FormatError {
    FormatError {
        line,
        message: message.to_owned(),
    }
}
