use sketchfold::build::{Stat, build};
use sketchfold::bundle::{Bundle, Entry};
use sketchfold::format::encode;
use sketchfold::frequent::Frequent;
use sketchfold::hist::Hist;
use sketchfold::hll::Hll;
use sketchfold::member::Member;
use sketchfold::sketch::Sketch;
use sketchfold::spec::{Kind, Spec, parse_spec};

fn spec(text: &str) -> Spec {
    parse_spec(text).expect("valid handwritten specification")
}

fn assert_invalid_spec(text: &str) {
    assert_eq!(parse_spec(text), None, "unexpectedly parsed {text:?}");
}

fn entry(name: &str, sketch: Sketch) -> Entry {
    Entry {
        name: name.to_owned(),
        sketch,
    }
}

#[test]
fn specs_supply_each_kind_default_and_construct_empty_sketches() {
    let distinct = spec("a:distinct:0");
    let top = spec("b:top:1");
    let hist = spec("c:hist:2");
    let member = spec("d:member:99");

    assert_eq!(
        distinct,
        Spec {
            name: "a".to_owned(),
            kind: Kind::Distinct,
            field: 0,
            param: 12
        }
    );
    assert_eq!(
        top,
        Spec {
            name: "b".to_owned(),
            kind: Kind::Top,
            field: 1,
            param: 16
        }
    );
    assert_eq!(
        hist,
        Spec {
            name: "c".to_owned(),
            kind: Kind::Hist,
            field: 2,
            param: 4
        }
    );
    assert_eq!(
        member,
        Spec {
            name: "d".to_owned(),
            kind: Kind::Member,
            field: 99,
            param: 14
        }
    );
    assert_eq!(distinct.new_sketch(), Sketch::Distinct(Hll::new(12)));
    assert_eq!(top.new_sketch(), Sketch::Top(Frequent::new(16)));
    assert_eq!(hist.new_sketch(), Sketch::Hist(Hist::new(4)));
    assert_eq!(member.new_sketch(), Sketch::Member(Member::new(14)));
    assert_eq!(distinct.new_sketch().n(), 0);
    assert_eq!(top.new_sketch().n(), 0);
    assert_eq!(hist.new_sketch().n(), 0);
    assert_eq!(member.new_sketch().n(), 0);
}

#[test]
fn specs_accept_inclusive_parameter_and_field_endpoints() {
    assert_eq!(spec("distinct:distinct:0:4").param, 4);
    assert_eq!(spec("distinct:distinct:99:16").param, 16);
    assert_eq!(spec("top:top:0:1").param, 1);
    assert_eq!(spec("top:top:99:1000").param, 1000);
    assert_eq!(spec("hist:hist:0:1").param, 1);
    assert_eq!(spec("hist:hist:99:8").param, 8);
    assert_eq!(spec("member:member:0:6").param, 6);
    assert_eq!(spec("member:member:99:20").param, 20);
    assert_eq!(spec("field0:top:0:1").field, 0);
    assert_eq!(spec("field99:top:99:1000").field, 99);
}

#[test]
fn specs_accept_name_lengths_one_and_thirty_two() {
    let one = spec("a:distinct:0");
    let thirty_two = spec("a1234567890123456789012345678901:member:0");

    assert_eq!(one.name, "a");
    assert_eq!(thirty_two.name, "a1234567890123456789012345678901");
    assert_eq!(thirty_two.name.len(), 32);
}

#[test]
fn specs_reject_invalid_names_and_unknown_kinds() {
    assert_invalid_spec(":top:0");
    assert_invalid_spec("A:top:0");
    assert_invalid_spec("1a:top:0");
    assert_invalid_spec("_a:top:0");
    assert_invalid_spec("aB:top:0");
    assert_invalid_spec("a.:top:0");
    assert_invalid_spec("aé:top:0");
    assert_invalid_spec("a12345678901234567890123456789012:top:0");
    assert_invalid_spec("a:unknown:0");
}

#[test]
fn specs_reject_missing_extra_and_empty_components() {
    assert_invalid_spec("");
    assert_invalid_spec("a");
    assert_invalid_spec("a:top");
    assert_invalid_spec("a:top:0:");
    assert_invalid_spec("a:top:0:1:extra");
    assert_invalid_spec("a::0");
    assert_invalid_spec("a:top:");
}

#[test]
fn specs_reject_noncanonical_or_out_of_range_fields() {
    assert_invalid_spec("a:top:00");
    assert_invalid_spec("a:top:01");
    assert_invalid_spec("a:top:100");
    assert_invalid_spec("a:top:-1");
    assert_invalid_spec("a:top:+1");
    assert_invalid_spec("a:top: 1");
    assert_invalid_spec("a:top:١");
    assert_invalid_spec("a:top:18446744073709551616");
}

#[test]
fn specs_reject_each_kind_parameter_outside_its_range() {
    assert_invalid_spec("a:distinct:0:3");
    assert_invalid_spec("a:distinct:0:17");
    assert_invalid_spec("a:top:0:0");
    assert_invalid_spec("a:top:0:1001");
    assert_invalid_spec("a:hist:0:0");
    assert_invalid_spec("a:hist:0:9");
    assert_invalid_spec("a:member:0:5");
    assert_invalid_spec("a:member:0:21");
}

#[test]
fn specs_reject_noncanonical_explicit_parameters() {
    assert_invalid_spec("a:distinct:0:04");
    assert_invalid_spec("a:top:0:01");
    assert_invalid_spec("a:hist:0:01");
    assert_invalid_spec("a:member:0:06");
    assert_invalid_spec("a:distinct:0:+4");
    assert_invalid_spec("a:top:0:-1");
    assert_invalid_spec("a:hist:0: 4");
    assert_invalid_spec("a:member:0:٦");
    assert_invalid_spec("a:top:0:18446744073709551616");
}

#[test]
fn empty_specs_and_empty_inputs_produce_empty_outputs() {
    assert_eq!(build(&[], b",", &[]), (Bundle { entries: vec![] }, vec![]));
    assert_eq!(
        build(&[], b",", &[b"one\n".to_vec()]),
        (Bundle { entries: vec![] }, vec![])
    );
    assert_eq!(
        build(&[spec("words:top:0:2")], b",", &[]),
        (
            Bundle {
                entries: vec![entry("words", Sketch::Top(Frequent::new(2)))]
            },
            vec![Stat {
                accepted: 0,
                skipped: 0
            }],
        )
    );
}

#[test]
fn build_preserves_spec_order_and_initializes_every_requested_sketch() {
    let specs = [
        spec("first:member:0:6"),
        spec("second:distinct:0:4"),
        spec("third:hist:0:1"),
    ];
    let (bundle, stats) = build(&specs, b" ", &[]);

    assert_eq!(
        bundle.entries,
        vec![
            entry("first", Sketch::Member(Member::new(6))),
            entry("second", Sketch::Distinct(Hll::new(4))),
            entry("third", Sketch::Hist(Hist::new(1))),
        ]
    );
    assert_eq!(
        stats,
        vec![
            Stat {
                accepted: 0,
                skipped: 0
            },
            Stat {
                accepted: 0,
                skipped: 0
            },
            Stat {
                accepted: 0,
                skipped: 0
            },
        ]
    );
}

#[test]
fn field_zero_selects_the_whole_line_and_field_one_selects_before_delimiter() {
    let specs = [spec("whole:top:0:2"), spec("first:top:1:2")];
    let inputs = [b"left:right\n".to_vec()];
    let (bundle, stats) = build(&specs, b":", &inputs);
    let mut whole = Frequent::new(2);
    whole.add(b"left:right");
    let mut first = Frequent::new(2);
    first.add(b"left");

    assert_eq!(bundle.get("whole"), Some(&Sketch::Top(whole)));
    assert_eq!(bundle.get("first"), Some(&Sketch::Top(first)));
    assert_eq!(
        stats,
        vec![
            Stat {
                accepted: 1,
                skipped: 0
            },
            Stat {
                accepted: 1,
                skipped: 0
            },
        ]
    );
}

#[test]
fn build_retains_repeated_delimiter_empty_fields_and_counts_each_sketch_independently() {
    let specs = [
        spec("whole:top:0:10"),
        spec("one:top:1:10"),
        spec("two:top:2:10"),
        spec("three:top:3:10"),
    ];
    let inputs = [b"alpha,,omega\n\nbeta,gamma,\n,value\n".to_vec()];
    let (bundle, stats) = build(&specs, b",", &inputs);

    assert_eq!(
        stats,
        vec![
            Stat {
                accepted: 3,
                skipped: 0
            },
            Stat {
                accepted: 2,
                skipped: 1
            },
            Stat {
                accepted: 2,
                skipped: 1
            },
            Stat {
                accepted: 1,
                skipped: 2
            },
        ]
    );
    assert_eq!(bundle.get("whole").unwrap().n(), 3);
    assert_eq!(bundle.get("one").unwrap().n(), 2);
    assert_eq!(bundle.get("two").unwrap().n(), 2);
    assert_eq!(bundle.get("three").unwrap().n(), 1);
    assert_eq!(
        bundle.get("two"),
        Some(&Sketch::Top({
            let mut expected = Frequent::new(10);
            expected.add(b"gamma");
            expected.add(b"value");
            expected
        }))
    );
    assert_eq!(
        bundle.get("three"),
        Some(&Sketch::Top({
            let mut expected = Frequent::new(10);
            expected.add(b"omega");
            expected
        }))
    );
}

#[test]
fn build_splits_multi_byte_delimiters_and_keeps_empty_middle_fields() {
    let specs = [
        spec("whole:top:0:4"),
        spec("left:top:1:4"),
        spec("middle:top:2:4"),
        spec("right:top:3:4"),
        spec("missing:top:4:4"),
    ];
    let (bundle, stats) = build(&specs, "§".as_bytes(), &["a§§b\n".as_bytes().to_vec()]);

    assert_eq!(
        stats,
        vec![
            Stat {
                accepted: 1,
                skipped: 0
            },
            Stat {
                accepted: 1,
                skipped: 0
            },
            Stat {
                accepted: 0,
                skipped: 1
            },
            Stat {
                accepted: 1,
                skipped: 0
            },
            Stat {
                accepted: 0,
                skipped: 1
            },
        ]
    );

    let mut whole = Frequent::new(4);
    whole.add("a§§b".as_bytes());
    let mut left = Frequent::new(4);
    left.add(b"a");
    let mut right = Frequent::new(4);
    right.add(b"b");
    assert_eq!(bundle.get("whole"), Some(&Sketch::Top(whole)));
    assert_eq!(bundle.get("left"), Some(&Sketch::Top(left)));
    assert_eq!(bundle.get("middle"), Some(&Sketch::Top(Frequent::new(4))));
    assert_eq!(bundle.get("right"), Some(&Sketch::Top(right)));
    assert_eq!(bundle.get("missing"), Some(&Sketch::Top(Frequent::new(4))));
}

#[test]
fn build_uses_upstream_lf_crlf_empty_line_and_unterminated_line_rules() {
    let specs = [spec("values:top:0:5")];
    let inputs = [b"left\r\nright\n\nthird\r".to_vec()];
    let (bundle, stats) = build(&specs, b" ", &inputs);
    let mut expected = Frequent::new(5);
    expected.add(b"left");
    expected.add(b"right");
    expected.add(b"third\r");

    assert_eq!(bundle.get("values"), Some(&Sketch::Top(expected)));
    assert_eq!(
        stats,
        vec![Stat {
            accepted: 3,
            skipped: 0
        }]
    );
}

#[test]
fn build_splits_each_unterminated_file_independently() {
    let specs = [spec("values:top:0:2")];
    let inputs = [b"left".to_vec(), b"right".to_vec()];
    let (bundle, stats) = build(&specs, b",", &inputs);
    let mut expected = Frequent::new(2);
    expected.add(b"left");
    expected.add(b"right");

    assert_eq!(bundle.get("values"), Some(&Sketch::Top(expected)));
    assert_eq!(
        stats,
        vec![Stat {
            accepted: 2,
            skipped: 0
        }]
    );
}

#[test]
fn build_keeps_invalid_utf8_and_nul_bytes_for_non_histogram_sketches() {
    let specs = [
        spec("top:top:0:4"),
        spec("distinct:distinct:0:4"),
        spec("member:member:0:6"),
        spec("hist:hist:0:1"),
    ];
    let inputs = [b"\xff\n\0\n7\n".to_vec()];
    let (bundle, stats) = build(&specs, b",", &inputs);

    let mut top = Frequent::new(4);
    top.add(b"\xff");
    top.add(b"\0");
    top.add(b"7");
    let mut distinct = Hll::new(4);
    distinct.add(b"\xff");
    distinct.add(b"\0");
    distinct.add(b"7");
    let mut member = Member::new(6);
    member.add(b"\xff");
    member.add(b"\0");
    member.add(b"7");
    let mut hist = Hist::new(1);
    hist.add(7);

    assert_eq!(
        bundle.entries,
        vec![
            entry("top", Sketch::Top(top)),
            entry("distinct", Sketch::Distinct(distinct)),
            entry("member", Sketch::Member(member)),
            entry("hist", Sketch::Hist(hist)),
        ]
    );
    assert_eq!(
        stats,
        vec![
            Stat {
                accepted: 3,
                skipped: 0
            },
            Stat {
                accepted: 3,
                skipped: 0
            },
            Stat {
                accepted: 3,
                skipped: 0
            },
            Stat {
                accepted: 1,
                skipped: 2
            },
        ]
    );
}

#[test]
fn histogram_skips_noncanonical_and_overflow_values_while_other_kinds_accept_them() {
    let specs = [
        spec("hist:hist:0:1"),
        spec("top:top:0:8"),
        spec("member:member:0:6"),
    ];
    let inputs = [b"0\n18446744073709551615\n01\n-1\n18446744073709551616\n".to_vec()];
    let (bundle, stats) = build(&specs, b" ", &inputs);
    let mut hist = Hist::new(1);
    hist.add(0);
    hist.add(u64::MAX);
    let mut top = Frequent::new(8);
    top.add(b"0");
    top.add(b"18446744073709551615");
    top.add(b"01");
    top.add(b"-1");
    top.add(b"18446744073709551616");
    let mut member = Member::new(6);
    member.add(b"0");
    member.add(b"18446744073709551615");
    member.add(b"01");
    member.add(b"-1");
    member.add(b"18446744073709551616");

    assert_eq!(
        bundle.entries,
        vec![
            entry("hist", Sketch::Hist(hist)),
            entry("top", Sketch::Top(top)),
            entry("member", Sketch::Member(member)),
        ]
    );
    assert_eq!(
        stats,
        vec![
            Stat {
                accepted: 2,
                skipped: 3
            },
            Stat {
                accepted: 5,
                skipped: 0
            },
            Stat {
                accepted: 5,
                skipped: 0
            },
        ]
    );
}

#[test]
fn access_log_build_matches_explicit_upstream_sketch_additions() {
    let access_log = concat!(
        "10.0.0.1 /index.html 200 12\n",
        "10.0.0.2 /index.html 200 15\n",
        "10.0.0.1 /about 200 40\n",
        "10.0.0.3 /index.html 304 3\n",
        "10.0.0.2 /api/items 500 950\n",
        "10.0.0.4 /index.html 200 18\n",
        "10.0.0.1 /api/items 200 120\n",
        "10.0.0.5 /about 404 7\n",
        "10.0.0.3 /index.html 200 14\n",
        "10.0.0.2 /api/items 200 130\n",
    );
    let specs = [
        spec("ips:distinct:1:4"),
        spec("paths:top:2:2"),
        spec("ms:hist:4:4"),
        spec("seen:member:2:6"),
    ];
    let (bundle, stats) = build(&specs, b" ", &[access_log.as_bytes().to_vec()]);

    let mut ips = Hll::new(4);
    ips.add(b"10.0.0.1");
    ips.add(b"10.0.0.2");
    ips.add(b"10.0.0.1");
    ips.add(b"10.0.0.3");
    ips.add(b"10.0.0.2");
    ips.add(b"10.0.0.4");
    ips.add(b"10.0.0.1");
    ips.add(b"10.0.0.5");
    ips.add(b"10.0.0.3");
    ips.add(b"10.0.0.2");
    let mut paths = Frequent::new(2);
    paths.add(b"/index.html");
    paths.add(b"/index.html");
    paths.add(b"/about");
    paths.add(b"/index.html");
    paths.add(b"/api/items");
    paths.add(b"/index.html");
    paths.add(b"/api/items");
    paths.add(b"/about");
    paths.add(b"/index.html");
    paths.add(b"/api/items");
    let mut milliseconds = Hist::new(4);
    milliseconds.add(12);
    milliseconds.add(15);
    milliseconds.add(40);
    milliseconds.add(3);
    milliseconds.add(950);
    milliseconds.add(18);
    milliseconds.add(120);
    milliseconds.add(7);
    milliseconds.add(14);
    milliseconds.add(130);
    let mut seen = Member::new(6);
    seen.add(b"/index.html");
    seen.add(b"/index.html");
    seen.add(b"/about");
    seen.add(b"/index.html");
    seen.add(b"/api/items");
    seen.add(b"/index.html");
    seen.add(b"/api/items");
    seen.add(b"/about");
    seen.add(b"/index.html");
    seen.add(b"/api/items");

    assert_eq!(
        bundle.entries,
        vec![
            entry("ips", Sketch::Distinct(ips)),
            entry("paths", Sketch::Top(paths)),
            entry("ms", Sketch::Hist(milliseconds)),
            entry("seen", Sketch::Member(seen)),
        ]
    );
    assert_eq!(
        stats,
        vec![
            Stat {
                accepted: 10,
                skipped: 0
            },
            Stat {
                accepted: 10,
                skipped: 0
            },
            Stat {
                accepted: 10,
                skipped: 0
            },
            Stat {
                accepted: 10,
                skipped: 0
            },
        ]
    );
}

#[test]
fn access_log_distinct_only_bundle_matches_the_canonical_worked_fixture() {
    let access_log = concat!(
        "10.0.0.1 /index.html 200 12\n",
        "10.0.0.2 /index.html 200 15\n",
        "10.0.0.1 /about 200 40\n",
        "10.0.0.3 /index.html 304 3\n",
        "10.0.0.2 /api/items 500 950\n",
        "10.0.0.4 /index.html 200 18\n",
        "10.0.0.1 /api/items 200 120\n",
        "10.0.0.5 /about 404 7\n",
        "10.0.0.3 /index.html 200 14\n",
        "10.0.0.2 /api/items 200 130\n",
    );
    let (bundle, stats) = build(
        &[spec("ips:distinct:1:4")],
        b" ",
        &[access_log.as_bytes().to_vec()],
    );

    assert_eq!(
        stats,
        vec![Stat {
            accepted: 10,
            skipped: 0
        }]
    );
    assert_eq!(
        encode(&bundle),
        concat!(
            "sketchfold 1\n",
            "sketch ips distinct p=4 n=10\n",
            "regs 01000000000000010100000200030000\n",
            "end c657f82f\n",
        ),
    );
}
