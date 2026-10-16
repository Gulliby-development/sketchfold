use sketchfold::bundle::{Bundle, Entry};
use sketchfold::error::FormatError;
use sketchfold::format::{decode, encode};
use sketchfold::frequent::Frequent;
use sketchfold::hash::checksum32;
use sketchfold::hist::Hist;
use sketchfold::hll::Hll;
use sketchfold::member::Member;
use sketchfold::sketch::Sketch;

fn with_checksum(prefix: &str) -> Vec<u8> {
    let mut bytes = prefix.as_bytes().to_vec();
    bytes.extend_from_slice(format!("end {}\n", checksum32(prefix.as_bytes())).as_bytes());
    bytes
}

fn assert_error(bytes: &[u8], line: usize, message: &str) {
    assert_eq!(
        decode(bytes),
        Err(FormatError {
            line,
            message: message.to_owned(),
        })
    );
}

fn assert_prefix_error(prefix: &str, line: usize, message: &str) {
    assert_error(&with_checksum(prefix), line, message);
}

fn entry(name: &str, sketch: Sketch) -> Entry {
    Entry {
        name: name.to_owned(),
        sketch,
    }
}

#[test]
fn encode_writes_canonical_all_kind_ascii_text_with_exact_checksum() {
    let mut top = Frequent::new(2);
    top.add(b"b");
    top.add(b"a");

    let bundle = Bundle {
        entries: vec![
            entry("empty", Sketch::Distinct(Hll::new(4))),
            entry("words", Sketch::Top(top)),
            entry("latencies", Sketch::Hist(Hist::new(1))),
            entry("membership", Sketch::Member(Member::new(6))),
        ],
    };
    let expected = concat!(
        "sketchfold 1\n",
        "sketch empty distinct p=4 n=0\n",
        "regs 00000000000000000000000000000000\n",
        "sketch words top k=2 n=2\n",
        "item 1 61\n",
        "item 1 62\n",
        "sketch latencies hist s=1 n=0 min=0 max=0\n",
        "sketch membership member b=6 n=0\n",
        "bits 0000000000000000\n",
        "end 36df9449\n",
    );

    let encoded = encode(&bundle);
    assert_eq!(encoded, expected);
    assert!(encoded.is_ascii());
    assert!(encoded.ends_with('\n'));
    assert_eq!(decode(encoded.as_bytes()), Ok(bundle));
}

#[test]
fn empty_bundle_has_exact_canonical_text_and_decodes() {
    let expected = "sketchfold 1\nend d79b2121\n";
    let encoded = encode(&Bundle { entries: vec![] });

    assert_eq!(encoded, expected);
    assert_eq!(decode(expected.as_bytes()), Ok(Bundle { entries: vec![] }));
}

#[test]
fn arbitrary_and_empty_top_values_round_trip_as_raw_bytes() {
    let mut top = Frequent::new(3);
    top.add(b"\xff");
    top.add(b"\x00");
    top.add(b"");
    let bundle = Bundle {
        entries: vec![entry("raw", Sketch::Top(top))],
    };

    let encoded = encode(&bundle);
    assert!(encoded.contains("item 1 \nitem 1 00\nitem 1 ff\n"));
    assert_eq!(decode(encoded.as_bytes()), Ok(bundle));
}

#[test]
fn encode_sorts_tied_top_items_by_byte_value() {
    let bundle = Bundle {
        entries: vec![entry(
            "words",
            Sketch::Top(Frequent {
                k: 2,
                n: 2,
                items: vec![(b"b".to_vec(), 1), (b"a".to_vec(), 1)],
            }),
        )],
    };
    let expected_prefix = "sketchfold 1\nsketch words top k=2 n=2\nitem 1 61\nitem 1 62\n";

    let encoded = encode(&bundle);
    assert_eq!(
        encoded,
        String::from_utf8(with_checksum(expected_prefix)).unwrap()
    );
    assert_eq!(
        decode(encoded.as_bytes()),
        Ok(Bundle {
            entries: vec![entry(
                "words",
                Sketch::Top(Frequent {
                    k: 2,
                    n: 2,
                    items: vec![(b"a".to_vec(), 1), (b"b".to_vec(), 1)],
                }),
            )],
        })
    );
}

#[test]
fn encode_sorts_histogram_buckets_by_shift_then_top() {
    let bundle = Bundle {
        entries: vec![entry(
            "latencies",
            Sketch::Hist(Hist {
                s: 1,
                n: 3,
                min: 1,
                max: 15,
                buckets: vec![(2, 3, 1), (0, 1, 2)],
            }),
        )],
    };
    let expected_prefix = concat!(
        "sketchfold 1\n",
        "sketch latencies hist s=1 n=3 min=1 max=15\n",
        "bucket 0 1 2\n",
        "bucket 2 3 1\n",
    );

    let encoded = encode(&bundle);
    assert_eq!(
        encoded,
        String::from_utf8(with_checksum(expected_prefix)).unwrap()
    );
    assert_eq!(
        decode(encoded.as_bytes()),
        Ok(Bundle {
            entries: vec![entry(
                "latencies",
                Sketch::Hist(Hist {
                    s: 1,
                    n: 3,
                    min: 1,
                    max: 15,
                    buckets: vec![(0, 1, 2), (2, 3, 1)],
                }),
            )],
        })
    );
}

#[test]
fn member_payload_uses_low_bit_first_nibbles_for_known_positions() {
    let mut member = Member::new(6);
    member.add(b"a");
    let bundle = Bundle {
        entries: vec![entry("seen", Sketch::Member(member))],
    };

    let encoded = encode(&bundle);
    assert!(encoded.contains("bits 0200400800000010\n"));
    assert_eq!(decode(encoded.as_bytes()), Ok(bundle));
}

#[test]
fn full_u64_counts_round_trip_without_truncation() {
    let bundle = Bundle {
        entries: vec![
            entry(
                "top",
                Sketch::Top(Frequent {
                    k: 1,
                    n: u64::MAX,
                    items: vec![(b"max".to_vec(), u64::MAX)],
                }),
            ),
            entry(
                "hist",
                Sketch::Hist(Hist {
                    s: 1,
                    n: u64::MAX,
                    min: u64::MAX,
                    max: u64::MAX,
                    buckets: vec![(62, 3, u64::MAX)],
                }),
            ),
        ],
    };

    let encoded = encode(&bundle);
    assert!(encoded.contains("n=18446744073709551615\n"));
    assert!(encoded.contains("item 18446744073709551615 6d6178\n"));
    assert!(encoded.contains("bucket 62 3 18446744073709551615\n"));
    assert_eq!(decode(encoded.as_bytes()), Ok(bundle));
}

#[test]
fn minimum_and_maximum_parameters_and_payload_sizes_round_trip() {
    let minimum = Bundle {
        entries: vec![
            entry("distinct", Sketch::Distinct(Hll::new(4))),
            entry("top", Sketch::Top(Frequent::new(1))),
            entry("hist", Sketch::Hist(Hist::new(1))),
            entry("member", Sketch::Member(Member::new(6))),
        ],
    };
    let maximum = Bundle {
        entries: vec![
            entry("distinct", Sketch::Distinct(Hll::new(16))),
            entry("top", Sketch::Top(Frequent::new(1000))),
            entry("hist", Sketch::Hist(Hist::new(8))),
            entry("member", Sketch::Member(Member::new(20))),
        ],
    };

    let minimum_text = encode(&minimum);
    let maximum_text = encode(&maximum);
    assert!(minimum_text.contains("distinct p=4 n=0\n"));
    assert!(minimum_text.contains("top k=1 n=0\n"));
    assert!(minimum_text.contains("hist s=1 n=0 min=0 max=0\n"));
    assert!(minimum_text.contains("member b=6 n=0\n"));
    assert!(maximum_text.contains("distinct p=16 n=0\n"));
    assert!(maximum_text.contains("top k=1000 n=0\n"));
    assert!(maximum_text.contains("hist s=8 n=0 min=0 max=0\n"));
    assert!(maximum_text.contains("member b=20 n=0\n"));
    assert_eq!(decode(minimum_text.as_bytes()), Ok(minimum));
    assert_eq!(decode(maximum_text.as_bytes()), Ok(maximum));
}

#[test]
fn decoder_accepts_register_rank_and_histogram_bucket_range_endpoints() {
    let prefix = concat!(
        "sketchfold 1\n",
        "sketch distinct distinct p=4 n=0\n",
        "regs 3d000000000000000000000000000000\n",
        "sketch hist hist s=1 n=4 min=0 max=18446744073709551615\n",
        "bucket 0 0 1\n",
        "bucket 0 3 1\n",
        "bucket 1 2 1\n",
        "bucket 62 3 1\n",
    );
    let bytes = with_checksum(prefix);

    assert_eq!(
        decode(&bytes),
        Ok(Bundle {
            entries: vec![
                entry(
                    "distinct",
                    Sketch::Distinct(Hll {
                        p: 4,
                        n: 0,
                        regs: vec![61, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                    }),
                ),
                entry(
                    "hist",
                    Sketch::Hist(Hist {
                        s: 1,
                        n: 4,
                        min: 0,
                        max: u64::MAX,
                        buckets: vec![(0, 0, 1), (0, 3, 1), (1, 2, 1), (62, 3, 1)],
                    }),
                ),
            ],
        })
    );
}

#[test]
fn empty_or_wrong_first_line_reports_not_a_bundle_on_line_one() {
    assert_error(b"", 1, "not a sketchfold bundle");
    assert_error(b"wrong header\n", 1, "not a sketchfold bundle");
    assert_error(b"sketchfold 1 \n", 1, "not a sketchfold bundle");
}

#[test]
fn malformed_sketch_headers_names_numbers_and_ranges_report_bad_sketch_line() {
    assert_prefix_error(
        "sketchfold 1\nsketch Upper top k=1 n=0\n",
        2,
        "bad sketch line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch abcdefghijklmnopqrstuvwxyzabcdefg top k=1 n=0\n",
        2,
        "bad sketch line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch 1x top k=1 n=0\n",
        2,
        "bad sketch line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch  x top k=1 n=0\n",
        2,
        "bad sketch line",
    );
    assert_prefix_error("sketchfold 1\nsketch x top k=1\n", 2, "bad sketch line");
    assert_prefix_error("sketchfold 1\nsketch x top k= n=0\n", 2, "bad sketch line");
    assert_prefix_error(
        "sketchfold 1\nsketch x top k=01 n=0\n",
        2,
        "bad sketch line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x top k=+1 n=0\n",
        2,
        "bad sketch line",
    );
    assert_prefix_error("sketchfold 1\nsketch x top k=١ n=0\n", 2, "bad sketch line");
    assert_prefix_error(
        "sketchfold 1\nsketch x top k=18446744073709551616 n=0\n",
        2,
        "bad sketch line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x distinct p=3 n=0\n",
        2,
        "bad sketch line",
    );
    assert_prefix_error("sketchfold 1\nsketch x top k=0 n=0\n", 2, "bad sketch line");
    assert_prefix_error(
        "sketchfold 1\nsketch x hist s=9 n=0 min=0 max=0\n",
        2,
        "bad sketch line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x member b=21 n=0\n",
        2,
        "bad sketch line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x other p=4 n=0\n",
        2,
        "bad sketch line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x top k=1 n=0 extra\n",
        2,
        "bad sketch line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x hist s=1 n=0 min=2 max=1\n",
        2,
        "bad sketch line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x hist s=1 n=0 min=0 max=1\n",
        2,
        "bad sketch line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x top k=1 n=01\n",
        2,
        "bad sketch line",
    );
}

#[test]
fn duplicate_valid_sketch_names_are_reported_at_the_repeated_header() {
    const NAME: &str = "same";
    let prefix = concat!(
        "sketchfold 1\n",
        "sketch same distinct p=4 n=0\n",
        "regs 00000000000000000000000000000000\n",
        "sketch same top k=1 n=0\n",
    );

    assert_prefix_error(prefix, 4, &format!("duplicate sketch name \"{NAME}\""));
}

#[test]
fn malformed_utf8_bom_nul_and_crlf_bytes_are_rejected_at_physical_lines() {
    assert_error(b"\xef\xbb\xbfsketchfold 1\n", 1, "not a sketchfold bundle");
    assert_error(b"sketchfold 1\r\n", 1, "not a sketchfold bundle");
    assert_error(b"sketchfold\0 1\n", 1, "not a sketchfold bundle");
    assert_error(
        b"sketchfold 1\nsketch x top k=1 n=1\nitem 1 \xff\n",
        3,
        "bad item line",
    );
    assert_error(
        b"sketchfold 1\nsketch x distinct p=4 n=0\nregs 0000000000000000000000000000000\0\n",
        3,
        "bad regs line",
    );
}

#[test]
fn bad_register_payload_length_hex_and_rank_report_the_payload_line() {
    assert_prefix_error(
        "sketchfold 1\nsketch x distinct p=4 n=0\nregs 00\n",
        3,
        "bad regs line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x distinct p=4 n=0\nregs 0000000000000000000000000000000G\n",
        3,
        "bad regs line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x distinct p=4 n=0\nregs 0000000000000000000000000000000A\n",
        3,
        "bad regs line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x distinct p=4 n=0\nregs 3e000000000000000000000000000000\n",
        3,
        "bad regs line",
    );
}

#[test]
fn bad_member_payload_length_and_hex_report_the_payload_line() {
    assert_prefix_error(
        "sketchfold 1\nsketch x member b=6 n=0\nbits 0\n",
        3,
        "bad bits line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x member b=6 n=0\nbits 000000000000000G\n",
        3,
        "bad bits line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x member b=6 n=0\nbits 000000000000000A\n",
        3,
        "bad bits line",
    );
}

#[test]
fn bad_item_count_hex_capacity_and_order_report_the_offending_line() {
    assert_prefix_error(
        "sketchfold 1\nsketch x top k=2 n=2\nitem 0 61\n",
        3,
        "bad item line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x top k=2 n=2\nitem 01 61\n",
        3,
        "bad item line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x top k=2 n=2\nitem 1 6A\n",
        3,
        "bad item line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x top k=2 n=2\nitem 1 6\n",
        3,
        "bad item line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x top k=1 n=2\nitem 1 61\nitem 1 62\n",
        4,
        "bad item line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x top k=2 n=2\nitem 1 61\nitem 2 62\n",
        4,
        "bad item line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x top k=2 n=2\nitem 1 62\nitem 1 61\n",
        4,
        "bad item line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x top k=2 n=2\nitem 1 61\nitem 1 61\n",
        4,
        "bad item line",
    );
}

#[test]
fn bad_bucket_count_shift_top_and_order_report_the_offending_line() {
    assert_prefix_error(
        "sketchfold 1\nsketch x hist s=1 n=1 min=0 max=1\nbucket 0 0 0\n",
        3,
        "bad bucket line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x hist s=1 n=1 min=0 max=1\nbucket 0 0 01\n",
        3,
        "bad bucket line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x hist s=1 n=1 min=0 max=1\nbucket 63 2 1\n",
        3,
        "bad bucket line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x hist s=1 n=1 min=0 max=1\nbucket 00 0 1\n",
        3,
        "bad bucket line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x hist s=1 n=1 min=0 max=1\nbucket 0 0 18446744073709551616\n",
        3,
        "bad bucket line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x hist s=1 n=1 min=0 max=1\nbucket 1 1 1\n",
        3,
        "bad bucket line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x hist s=1 n=1 min=0 max=1\nbucket 0 4 1\n",
        3,
        "bad bucket line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x hist s=1 n=2 min=0 max=1\nbucket 0 1 1\nbucket 0 0 1\n",
        4,
        "bad bucket line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x hist s=1 n=2 min=0 max=1\nbucket 0 1 1\nbucket 0 1 1\n",
        4,
        "bad bucket line",
    );
}

#[test]
fn wrong_kind_duplicate_payload_blank_and_unknown_lines_are_unexpected() {
    assert_prefix_error(
        "sketchfold 1\nsketch x hist s=1 n=0 min=0 max=0\nitem 1 61\n",
        3,
        "unexpected line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x distinct p=4 n=0\nregs 00000000000000000000000000000000\nregs 00000000000000000000000000000000\n",
        4,
        "unexpected line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x top k=1 n=0\n\n",
        3,
        "unexpected line",
    );
    assert_prefix_error(
        "sketchfold 1\nsketch x top k=1 n=0\nnote value\n",
        3,
        "unexpected line",
    );
    assert_error(b"sketchfold 1\nother\n", 2, "unexpected line");
}

#[test]
fn missing_register_and_bit_payloads_fail_at_sketch_end_and_eof_boundaries() {
    assert_error(
        b"sketchfold 1\nsketch x distinct p=4 n=0\nsketch y top k=1 n=0\n",
        3,
        "bad regs line",
    );
    assert_error(
        b"sketchfold 1\nsketch x distinct p=4 n=0\nend 00000000\n",
        3,
        "bad regs line",
    );
    assert_error(
        b"sketchfold 1\nsketch x distinct p=4 n=0\n",
        3,
        "bad regs line",
    );
    assert_error(
        b"sketchfold 1\nsketch x member b=6 n=0\nsketch y top k=1 n=0\n",
        3,
        "bad bits line",
    );
    assert_error(
        b"sketchfold 1\nsketch x member b=6 n=0\nend 00000000\n",
        3,
        "bad bits line",
    );
    assert_error(
        b"sketchfold 1\nsketch x member b=6 n=0\n",
        3,
        "bad bits line",
    );
}

#[test]
fn top_aggregate_errors_precede_invalid_headers_end_lines_and_missing_end() {
    assert_error(
        b"sketchfold 1\nsketch x top k=1 n=0\nitem 1 61\nsketch Bad top k=1 n=0\n",
        2,
        "counts exceed n",
    );
    assert_error(
        b"sketchfold 1\nsketch x top k=1 n=0\nitem 1 61\nend 00000000\n",
        2,
        "counts exceed n",
    );
    assert_error(
        b"sketchfold 1\nsketch x top k=1 n=0\nitem 1 61\nsketch x top k=1 n=0\n",
        2,
        "counts exceed n",
    );
    assert_error(
        b"sketchfold 1\nsketch x top k=1 n=0\nitem 1 61\n",
        2,
        "counts exceed n",
    );
}

#[test]
fn top_count_totals_above_n_and_u64_max_are_rejected_at_the_header() {
    assert_prefix_error(
        "sketchfold 1\nsketch x top k=1 n=0\nitem 1 61\n",
        2,
        "counts exceed n",
    );
    assert_prefix_error(
        concat!(
            "sketchfold 1\n",
            "sketch x top k=2 n=18446744073709551615\n",
            "item 18446744073709551615 61\n",
            "item 1 62\n",
        ),
        2,
        "counts exceed n",
    );
}

#[test]
fn histogram_aggregate_mismatch_and_totals_above_u64_max_are_rejected() {
    assert_prefix_error(
        "sketchfold 1\nsketch x hist s=1 n=2 min=0 max=1\nbucket 0 0 1\n",
        2,
        "bucket counts do not add up to n",
    );
    assert_prefix_error(
        concat!(
            "sketchfold 1\n",
            "sketch x hist s=1 n=18446744073709551615 min=0 max=1\n",
            "bucket 0 0 18446744073709551615\n",
            "bucket 0 1 1\n",
        ),
        2,
        "bucket counts do not add up to n",
    );
    assert_error(
        b"sketchfold 1\nsketch x hist s=1 n=2 min=0 max=1\nbucket 0 0 1\nsketch Bad top k=1 n=0\n",
        2,
        "bucket counts do not add up to n",
    );
}

#[test]
fn end_syntax_is_checked_before_checksum_and_requires_final_lf() {
    assert_error(b"sketchfold 1\nend 0000000G\n", 2, "bad end line");
    assert_error(b"sketchfold 1\nend 0000000\n", 2, "bad end line");
    assert_error(
        b"sketchfold 1\nend 0000000G\nunexpected\n",
        2,
        "bad end line",
    );
    assert_error(b"sketchfold 1\nend ABCDEF12\n", 2, "bad end line");
    assert_error(b"sketchfold 1\nend 00000000 extra\n", 2, "bad end line");
    assert_error(b"sketchfold 1\nend 00000000", 2, "bad end line");
    assert_error(b"sketchfold 1\nend\n", 2, "unexpected line");
}

#[test]
fn well_formed_but_wrong_checksum_reports_checksum_mismatch_at_end() {
    assert_ne!(checksum32(b"sketchfold 1\n"), "00000000");
    assert_error(b"sketchfold 1\nend 00000000\n", 2, "checksum mismatch");
    assert_error(
        b"sketchfold 1\nend 00000000\nunexpected\n",
        2,
        "checksum mismatch",
    );
}

#[test]
fn every_physical_line_after_end_including_blank_data_is_rejected() {
    let mut with_data = with_checksum("sketchfold 1\n");
    with_data.extend_from_slice(b"unexpected\n");
    assert_error(&with_data, 3, "unexpected data after end");

    let mut with_blank = with_checksum("sketchfold 1\n");
    with_blank.extend_from_slice(b"\n");
    assert_error(&with_blank, 3, "unexpected data after end");
}

#[test]
fn missing_end_uses_physical_line_count_plus_one_after_valid_sketches() {
    assert_error(b"sketchfold 1\n", 2, "missing end line");
    assert_error(
        b"sketchfold 1\nsketch x distinct p=4 n=0\nregs 00000000000000000000000000000000\n",
        4,
        "missing end line",
    );
}

#[test]
fn changed_worked_example_count_reports_checksum_mismatch_on_actual_line_four() {
    let fixture = concat!(
        "sketchfold 1\n",
        "sketch ips distinct p=4 n=11\n",
        "regs 01000000000000010100000200030000\n",
        "end c657f82f\n",
    );

    assert_error(fixture.as_bytes(), 4, "checksum mismatch");
}
