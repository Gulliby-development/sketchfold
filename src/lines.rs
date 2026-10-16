pub fn split_lines(bytes: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut start = 0;

    for (index, &byte) in bytes.iter().enumerate() {
        if byte == b'\n' {
            let mut end = index;
            if end > start && bytes[end - 1] == b'\r' {
                end -= 1;
            }
            if start < end {
                lines.push(&bytes[start..end]);
            }
            start = index + 1;
        }
    }

    if start < bytes.len() {
        lines.push(&bytes[start..]);
    }

    lines
}

pub fn field<'a>(line: &'a [u8], delim: &[u8], n: usize) -> Option<&'a [u8]> {
    if n == 0 {
        return Some(line);
    }
    if delim.is_empty() {
        return None;
    }

    let mut start = 0;
    let mut field_number = n;

    loop {
        let remaining = &line[start..];
        let Some(offset) = remaining
            .windows(delim.len())
            .position(|window| window == delim)
        else {
            return (field_number == 1).then_some(remaining);
        };

        let end = start + offset;
        if field_number == 1 {
            return Some(&line[start..end]);
        }

        field_number -= 1;
        start = end + delim.len();
    }
}
