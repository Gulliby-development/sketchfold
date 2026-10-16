use sketchfold::error::{CliError, FormatError};
use sketchfold::hash::{checksum32, fnv64, hash64};
use sketchfold::lines::{field, split_lines};
use sketchfold::text::{escape_value, hex_decode, hex_encode, parse_count, parse_u64, plural};

#[test]
fn cli_error_keeps_its_public_message() {
    let error = CliError("plain message".to_owned());
    assert_eq!(error.0, "plain message");
}

#[test]
fn format_error_keeps_its_public_fields() {
    let error = FormatError {
        line: 17,
        message: "bad record".to_owned(),
    };
    assert_eq!(error.line, 17);
    assert_eq!(error.message, "bad record");
}

#[test]
fn plural_uses_singular_only_for_one() {
    assert_eq!(plural(1, "value", "values"), "1 value");
}

#[test]
fn plural_uses_plural_for_zero_and_two() {
    assert_eq!(plural(0, "value", "values"), "0 values");
    assert_eq!(plural(2, "value", "values"), "2 values");
}

#[test]
fn plural_handles_the_largest_count() {
    assert_eq!(
        plural(u64::MAX, "value", "values"),
        "18446744073709551615 values"
    );
}

#[test]
fn parse_u64_accepts_zero_and_one_digit_values() {
    assert_eq!(parse_u64(b"0"), Some(0));
    assert_eq!(parse_u64(b"7"), Some(7));
}

#[test]
fn parse_u64_accepts_decimal_values_and_u64_max() {
    assert_eq!(parse_u64(b"42"), Some(42));
    assert_eq!(parse_u64(b"18446744073709551615"), Some(u64::MAX));
}

#[test]
fn parse_u64_rejects_overflow() {
    assert_eq!(parse_u64(b"18446744073709551616"), None);
}

#[test]
fn parse_u64_rejects_empty_leading_zero_and_signed_forms() {
    assert_eq!(parse_u64(b""), None);
    assert_eq!(parse_u64(b"00"), None);
    assert_eq!(parse_u64(b"01"), None);
    assert_eq!(parse_u64(b"+1"), None);
    assert_eq!(parse_u64(b"-1"), None);
}

#[test]
fn parse_u64_rejects_whitespace_and_non_digits() {
    assert_eq!(parse_u64(b" 1"), None);
    assert_eq!(parse_u64(b"1\n"), None);
    assert_eq!(parse_u64(b"1x"), None);
}

#[test]
fn parse_u64_rejects_non_ascii_digits_and_invalid_utf8() {
    assert_eq!(parse_u64("١".as_bytes()), None);
    assert_eq!(parse_u64(b"\xff"), None);
}

#[test]
fn parse_count_accepts_both_inclusive_bounds() {
    assert_eq!(parse_count("4", 4, 9), Some(4));
    assert_eq!(parse_count("9", 4, 9), Some(9));
}

#[test]
fn parse_count_accepts_values_inside_the_bounds() {
    assert_eq!(parse_count("6", 4, 9), Some(6));
}

#[test]
fn parse_count_rejects_values_outside_the_bounds() {
    assert_eq!(parse_count("3", 4, 9), None);
    assert_eq!(parse_count("10", 4, 9), None);
}

#[test]
fn parse_count_rejects_reversed_bounds() {
    assert_eq!(parse_count("4", 9, 4), None);
}

#[test]
fn parse_count_rejects_noncanonical_decimal_text() {
    assert_eq!(parse_count("", 0, u64::MAX), None);
    assert_eq!(parse_count("00", 0, u64::MAX), None);
    assert_eq!(parse_count("+1", 0, u64::MAX), None);
    assert_eq!(parse_count("18446744073709551616", 0, u64::MAX), None);
}

#[test]
fn hex_encode_handles_empty_and_arbitrary_bytes() {
    assert_eq!(hex_encode(b""), "");
    assert_eq!(hex_encode(&[0x00, 0x0f, 0x10, 0xff]), "000f10ff");
}

#[test]
fn hex_decode_accepts_empty_text() {
    assert_eq!(hex_decode(""), Some(Vec::new()));
}

#[test]
fn hex_decode_preserves_each_decoded_byte() {
    assert_eq!(hex_decode("000f10ff"), Some(vec![0x00, 0x0f, 0x10, 0xff]));
}

#[test]
fn hex_decode_rejects_uppercase_odd_length_and_prefixes() {
    assert_eq!(hex_decode("FF"), None);
    assert_eq!(hex_decode("f"), None);
    assert_eq!(hex_decode("0x00"), None);
}

#[test]
fn hex_decode_rejects_whitespace_invalid_digits_and_unicode() {
    assert_eq!(hex_decode("00 "), None);
    assert_eq!(hex_decode("gg"), None);
    assert_eq!(hex_decode("é0"), None);
}

#[test]
fn escape_value_preserves_empty_text_space_and_unicode() {
    assert_eq!(escape_value(b""), "");
    assert_eq!(escape_value(b"plain text"), "plain text");
    assert_eq!(escape_value("café".as_bytes()), "café");
}

#[test]
fn escape_value_doubles_backslashes() {
    assert_eq!(escape_value(b"a\\b"), "a\\\\b");
}

#[test]
fn escape_value_uses_lowercase_hex_for_each_control_boundary() {
    assert_eq!(escape_value(b"\0"), "\\x00");
    assert_eq!(escape_value(b"\t"), "\\x09");
    assert_eq!(escape_value(b"\n"), "\\x0a");
    assert_eq!(escape_value(b"\r"), "\\x0d");
    assert_eq!(escape_value(b"\x1f"), "\\x1f");
    assert_eq!(escape_value(b"\x7f"), "\\x7f");
}

#[test]
fn escape_value_replaces_an_invalid_utf8_byte() {
    assert_eq!(escape_value(b"a\xffb"), "a�b");
}

#[test]
fn escape_value_replaces_maximal_invalid_utf8_subparts() {
    assert_eq!(escape_value(&[0xe2, 0x82]), "�");
    assert_eq!(escape_value(&[0xe2, b'(', 0xa1]), "�(�");
    assert_eq!(escape_value(&[0xf0, 0x90, 0x80, b'A']), "�A");
    assert_eq!(escape_value(&[0xff, 0xfe]), "��");
}

#[test]
fn fnv64_matches_the_empty_input_vector() {
    assert_eq!(fnv64(b""), 0xcbf29ce484222325);
}

#[test]
fn fnv64_matches_the_a_vector() {
    assert_eq!(fnv64(b"a"), 0xaf63dc4c8601ec8c);
}

#[test]
fn hash64_matches_the_empty_input_vector() {
    assert_eq!(hash64(b""), 0xf52a15e9a9b5e89b);
}

#[test]
fn hash64_matches_the_a_vector() {
    assert_eq!(hash64(b"a"), 0x02c0bdbf481420f8);
}

#[test]
fn checksum32_matches_the_a_vector() {
    assert_eq!(checksum32(b"a"), "e40c292c");
}

#[test]
fn checksum32_matches_the_foobar_vector() {
    assert_eq!(checksum32(b"foobar"), "bf9cf968");
}

#[test]
fn checksum32_matches_the_empty_input_vector() {
    assert_eq!(checksum32(b""), "811c9dc5");
}

#[test]
fn checksum32_pads_a_leading_zero_digit() {
    assert_eq!(checksum32(b"\0"), "050c5d1f");
}

#[test]
fn split_lines_omits_empty_input_and_empty_lf_lines() {
    assert_eq!(split_lines(b""), Vec::<&[u8]>::new());
    assert_eq!(split_lines(b"\n"), Vec::<&[u8]>::new());
}

#[test]
fn split_lines_splits_lf_and_omits_empty_lines() {
    assert_eq!(
        split_lines(b"a\n\nb\n"),
        vec![b"a".as_slice(), b"b".as_slice()]
    );
}

#[test]
fn split_lines_removes_one_cr_before_lf_and_omits_empty_results() {
    assert_eq!(split_lines(b"a\r\n"), vec![b"a".as_slice()]);
    assert_eq!(split_lines(b"\r\n"), Vec::<&[u8]>::new());
    assert_eq!(split_lines(b"a\r\r\n"), vec![b"a\r".as_slice()]);
}

#[test]
fn split_lines_retains_bare_cr_and_unterminated_lines() {
    assert_eq!(split_lines(b"a\r"), vec![b"a\r".as_slice()]);
    assert_eq!(split_lines(b"a\nb"), vec![b"a".as_slice(), b"b".as_slice()]);
}

#[test]
fn split_lines_preserves_invalid_utf8_bytes() {
    assert_eq!(split_lines(b"a\xff\n"), vec![&b"a\xff"[..]]);
}

#[test]
fn field_zero_returns_the_whole_line() {
    assert_eq!(field(b" a  b ", b" ", 0), Some(&b" a  b "[..]));
}

#[test]
fn field_preserves_empty_parts_and_distinguishes_missing_parts() {
    let line = b" a  b ";
    assert_eq!(field(line, b" ", 1), Some(&b""[..]));
    assert_eq!(field(line, b" ", 2), Some(&b"a"[..]));
    assert_eq!(field(line, b" ", 3), Some(&b""[..]));
    assert_eq!(field(line, b" ", 4), Some(&b"b"[..]));
    assert_eq!(field(line, b" ", 5), Some(&b""[..]));
    assert_eq!(field(line, b" ", 6), None);
}

#[test]
fn field_returns_empty_parts_on_an_empty_line() {
    assert_eq!(field(b"", b",", 0), Some(&b""[..]));
    assert_eq!(field(b"", b",", 1), Some(&b""[..]));
    assert_eq!(field(b"", b",", 2), None);
}

#[test]
fn field_splits_multibyte_delimiters_without_decoding_line_bytes() {
    let line = b"\xffa\xc3\xa9\xfevalue";
    assert_eq!(field(line, "é".as_bytes(), 1), Some(&b"\xffa"[..]));
    assert_eq!(field(line, "é".as_bytes(), 2), Some(&b"\xfevalue"[..]));
}

#[test]
fn field_with_an_empty_delimiter_only_has_field_zero() {
    assert_eq!(field(b"line", b"", 0), Some(&b"line"[..]));
    assert_eq!(field(b"line", b"", 1), None);
}

#[test]
fn field_handles_nonoverlapping_delimiters() {
    assert_eq!(field(b"aaaa", b"aa", 1), Some(&b""[..]));
    assert_eq!(field(b"aaaa", b"aa", 2), Some(&b""[..]));
    assert_eq!(field(b"aaaa", b"aa", 3), Some(&b""[..]));
    assert_eq!(field(b"aaaa", b"aa", 4), None);
}

#[test]
fn field_returns_none_for_usize_max_on_a_short_line() {
    assert_eq!(field(b"a:b", b":", usize::MAX), None);
}

#[test]
fn field_result_borrows_the_line_not_the_delimiter() {
    let line = b"first:second";
    let second = {
        let delimiter = vec![b':'];
        field(line, &delimiter, 2).unwrap()
    };
    assert_eq!(second, b"second");
}
