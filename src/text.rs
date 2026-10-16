pub fn plural(n: u64, one: &str, many: &str) -> String {
    let word = if n == 1 { one } else { many };
    format!("{n} {word}")
}

pub fn parse_u64(bytes: &[u8]) -> Option<u64> {
    let (&first, rest) = bytes.split_first()?;
    if first == b'0' {
        return rest.is_empty().then_some(0);
    }
    if !first.is_ascii_digit() {
        return None;
    }

    let mut value = u64::from(first - b'0');
    for &byte in rest {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(u64::from(byte - b'0'))?;
    }

    Some(value)
}

pub fn parse_count(text: &str, lo: u64, hi: u64) -> Option<u64> {
    if lo > hi {
        return None;
    }

    let value = parse_u64(text.as_bytes())?;
    (lo..=hi).contains(&value).then_some(value)
}

pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

pub fn hex_decode(text: &str) -> Option<Vec<u8>> {
    fn nibble(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        }
    }

    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return None;
    }

    let mut decoded = Vec::with_capacity(bytes.len() / 2);
    let (pairs, _) = bytes.as_chunks::<2>();
    for pair in pairs {
        let high = nibble(pair[0])?;
        let low = nibble(pair[1])?;
        decoded.push((high << 4) | low);
    }
    Some(decoded)
}

pub fn escape_value(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let decoded = String::from_utf8_lossy(bytes);
    let mut escaped = String::with_capacity(decoded.len());
    for character in decoded.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '\u{0000}'..='\u{001f}' | '\u{007f}' => {
                let value = character as usize;
                escaped.push_str("\\x");
                escaped.push(char::from(HEX[(value >> 4) & 0x0f]));
                escaped.push(char::from(HEX[value & 0x0f]));
            }
            _ => escaped.push(character),
        }
    }
    escaped
}
