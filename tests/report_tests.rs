use std::cell::RefCell;
use std::collections::HashMap;

use sketchfold::bundle::{Bundle, Entry};
use sketchfold::cli::{Io, run};
use sketchfold::format::encode;
use sketchfold::frequent::Frequent;
use sketchfold::hist::Hist;
use sketchfold::hll::Hll;
use sketchfold::member::Member;
use sketchfold::report::{parse_q, show, top};
use sketchfold::sketch::Sketch;

fn entry(name: &str, sketch: Sketch) -> Entry {
    Entry {
        name: name.to_owned(),
        sketch,
    }
}

fn bundle(entries: Vec<Entry>) -> Bundle {
    Bundle { entries }
}

struct MemoryIo {
    files: HashMap<String, Vec<u8>>,
    stdout: RefCell<String>,
    stderr: RefCell<String>,
}

impl MemoryIo {
    fn with_top_bundle() -> Self {
        let bundle = bundle(vec![entry(
            "words",
            Sketch::Top(Frequent {
                k: 2,
                n: 3,
                items: vec![(b"alpha".to_vec(), 2), (b"beta".to_vec(), 1)],
            }),
        )]);
        Self {
            files: HashMap::from([("bundle.sk".to_owned(), encode(&bundle).into_bytes())]),
            stdout: RefCell::new(String::new()),
            stderr: RefCell::new(String::new()),
        }
    }
}

impl Io for MemoryIo {
    fn read_file(&self, path: &str) -> Option<Vec<u8>> {
        self.files.get(path).cloned()
    }

    fn read_stdin(&self) -> Vec<u8> {
        Vec::new()
    }

    fn write_file(&self, _path: &str, _data: &[u8]) -> bool {
        false
    }

    fn out(&self, text: &str) {
        self.stdout.borrow_mut().push_str(text);
    }

    fn err(&self, text: &str) {
        self.stderr.borrow_mut().push_str(text);
    }
}

fn run_top_with_limit(limit: &str) -> (i32, String, String) {
    let io = MemoryIo::with_top_bundle();
    let args = vec![
        "top".to_owned(),
        format!("--limit={limit}"),
        "bundle.sk".to_owned(),
        "words".to_owned(),
    ];
    let status = run(&args, &io);
    (
        status,
        io.stdout.borrow().clone(),
        io.stderr.borrow().clone(),
    )
}

#[test]
fn empty_bundle_renders_no_lines() {
    assert_eq!(show(&Bundle::default()), "");
}

#[test]
fn show_aligns_mixed_sketch_kinds_and_names_without_trailing_spaces() {
    let mut frequent = Frequent::new(2);
    frequent.add(b"alpha");
    frequent.add(b"alpha");
    frequent.add(b"beta");

    let mut histogram = Hist::new(4);
    histogram.add(3);
    histogram.add(15);
    histogram.add(950);

    let mut member = Member::new(6);
    member.add(b"alpha");

    assert_eq!(
        show(&bundle(vec![
            entry("ip", Sketch::Distinct(Hll::new(4))),
            entry("top", Sketch::Top(frequent)),
            entry("ms", Sketch::Hist(histogram)),
            entry("seen", Sketch::Member(member)),
        ])),
        "ip    distinct  p=4  n=0  about 0 distinct, error 26.0%\n\
         top   top       k=2  n=3  2 counters, slack 0\n\
         ms    hist      s=4  n=3  min 3, max 950, p50 15, p90 950, p99 950\n\
         seen  member    b=6  n=1  4 of 64 bits set\n"
    );
}

#[test]
fn show_uses_singular_and_plural_counter_details() {
    let one = Frequent {
        k: 1,
        n: 1,
        items: vec![(b"only".to_vec(), 1)],
    };
    let none = Frequent::new(1);

    assert_eq!(
        show(&bundle(vec![
            entry("one", Sketch::Top(one)),
            entry("none", Sketch::Top(none)),
        ])),
        "one   top       k=1  n=1  1 counter, slack 0\n\
         none  top       k=1  n=0  0 counters, slack 0\n"
    );
}

#[test]
fn show_reports_empty_and_nonempty_histogram_quantiles() {
    let mut histogram = Hist::new(2);
    histogram.add(1);
    histogram.add(2);
    histogram.add(8);

    assert_eq!(
        show(&bundle(vec![
            entry("empty", Sketch::Hist(Hist::new(2))),
            entry("full", Sketch::Hist(histogram)),
        ])),
        "empty  hist      s=2  n=0  empty\nfull   hist      s=2  n=3  min 1, max 8, p50 2, p90 8, p99 8\n"
    );
}

#[test]
fn show_formats_hyperloglog_error_to_one_decimal_place() {
    assert_eq!(
        show(&bundle(vec![entry("wide", Sketch::Distinct(Hll::new(12)))])),
        "wide  distinct  p=12  n=0  about 0 distinct, error 1.6%\n"
    );
    assert_eq!(
        show(&bundle(vec![entry("small", Sketch::Distinct(Hll::new(4)))])),
        "small  distinct  p=4  n=0  about 0 distinct, error 26.0%\n"
    );
}

#[test]
fn show_counts_set_member_bits_instead_of_insertions() {
    let mut member = Member::new(6);
    member.add(b"a");
    member.add(b"a");

    assert_eq!(
        show(&bundle(vec![entry("seen", Sketch::Member(member))])),
        "seen  member    b=6  n=2  4 of 64 bits set\n"
    );
}

#[test]
fn show_renders_full_u64_state_without_overflowing_reports() {
    let maximum = u64::MAX;
    let frequent = Frequent {
        k: 1,
        n: maximum,
        items: vec![(b"largest".to_vec(), maximum)],
    };
    let histogram = Hist {
        s: 1,
        n: maximum,
        min: maximum,
        max: maximum,
        buckets: vec![(62, 3, maximum)],
    };

    assert_eq!(
        show(&bundle(vec![
            entry("top", Sketch::Top(frequent)),
            entry("hist", Sketch::Hist(histogram)),
        ])),
        "top   top       k=1  n=18446744073709551615  1 counter, slack 0\n\
         hist  hist      s=1  n=18446744073709551615  min 18446744073709551615, max 18446744073709551615, p50 18446744073709551615, p90 18446744073709551615, p99 18446744073709551615\n"
    );
}

#[test]
fn top_renders_header_and_all_counters_with_count_width_and_value_order() {
    let frequent = Frequent {
        k: 3,
        n: 123,
        items: vec![
            (b"alpha".to_vec(), 100),
            (b"beta".to_vec(), 20),
            (b"gamma".to_vec(), 3),
        ],
    };

    assert_eq!(
        top("words", &frequent, None),
        "words: n=123, slack 0\n100  alpha\n 20  beta\n  3  gamma\n"
    );
}

#[test]
fn top_limit_preserves_largest_counter_alignment_and_zero_limit_keeps_header() {
    let frequent = Frequent {
        k: 2,
        n: 1239,
        items: vec![(b"large".to_vec(), 1234), (b"small".to_vec(), 2)],
    };

    assert_eq!(
        top("words", &frequent, Some(1)),
        "words: n=1239, slack 1\n1234  large\n"
    );
    assert_eq!(top("words", &frequent, Some(0)), "words: n=1239, slack 1\n");
}

#[test]
fn top_empty_items_keep_header_and_use_a_nonzero_count_width() {
    assert_eq!(
        top("empty", &Frequent::new(2), None),
        "empty: n=0, slack 0\n"
    );
}

#[test]
fn top_escapes_backslashes_controls_and_invalid_utf8_through_the_shared_helper() {
    let frequent = Frequent {
        k: 1,
        n: 1,
        items: vec![(b"a\\\n\0\x7f".to_vec(), 1)],
    };
    let invalid_utf8 = Frequent {
        k: 1,
        n: 1,
        items: vec![(vec![b'a', 0xff, b'b'], 1)],
    };

    assert_eq!(
        top("escape", &frequent, None),
        "escape: n=1, slack 0\n1  a\\\\\\x0a\\x00\\x7f\n"
    );
    assert_eq!(
        top("utf8", &invalid_utf8, None),
        "utf8: n=1, slack 0\n1  a�b\n"
    );
}

#[test]
fn parse_q_accepts_canonical_percentages_and_returns_thousandths_exactly() {
    assert_eq!(parse_q("0"), Some(0));
    assert_eq!(parse_q("0.001"), Some(1));
    assert_eq!(parse_q("0.01"), Some(10));
    assert_eq!(parse_q("0.1"), Some(100));
    assert_eq!(parse_q("50"), Some(50_000));
    assert_eq!(parse_q("50.000"), Some(50_000));
    assert_eq!(parse_q("99.9"), Some(99_900));
    assert_eq!(parse_q("100"), Some(100_000));
    assert_eq!(parse_q("100.000"), Some(100_000));
}

#[test]
fn parse_q_rejects_noncanonical_integer_forms_and_out_of_range_values() {
    assert_eq!(parse_q(""), None);
    assert_eq!(parse_q("00"), None);
    assert_eq!(parse_q("01"), None);
    assert_eq!(parse_q("101"), None);
    assert_eq!(parse_q("+1"), None);
    assert_eq!(parse_q("-1"), None);
    assert_eq!(parse_q(" 1"), None);
    assert_eq!(parse_q("1 "), None);
    assert_eq!(parse_q("1e2"), None);
    assert_eq!(parse_q("١"), None);
}

#[test]
fn parse_q_requires_one_to_three_fraction_digits_and_rejects_overflow_percentages() {
    assert_eq!(parse_q("."), None);
    assert_eq!(parse_q(".5"), None);
    assert_eq!(parse_q("1."), None);
    assert_eq!(parse_q("1.0000"), None);
    assert_eq!(parse_q("1.2.3"), None);
    assert_eq!(parse_q("100.001"), None);
    assert_eq!(parse_q("18446744073709551616"), None);
}

#[test]
fn top_limit_accepts_the_minimum_and_maximum_with_exact_output_and_status() {
    assert_eq!(
        run_top_with_limit("1"),
        (
            0,
            "words: n=3, slack 0\n2  alpha\n".to_owned(),
            String::new()
        )
    );
    assert_eq!(
        run_top_with_limit("1000000"),
        (
            0,
            "words: n=3, slack 0\n2  alpha\n1  beta\n".to_owned(),
            String::new()
        )
    );
}

#[test]
fn top_limit_zero_returns_the_exact_cli_error_and_status() {
    assert_eq!(
        run_top_with_limit("0"),
        (
            2,
            String::new(),
            "sketchfold: invalid --limit \"0\"\n".to_owned()
        )
    );
}

#[test]
fn top_limit_above_maximum_returns_the_exact_cli_error_and_status() {
    assert_eq!(
        run_top_with_limit("1000001"),
        (
            2,
            String::new(),
            "sketchfold: invalid --limit \"1000001\"\n".to_owned()
        )
    );
}

#[test]
fn top_limit_rejects_noncanonical_and_overflow_values_with_exact_cli_errors() {
    assert_eq!(
        run_top_with_limit("01"),
        (
            2,
            String::new(),
            "sketchfold: invalid --limit \"01\"\n".to_owned()
        )
    );
    assert_eq!(
        run_top_with_limit("18446744073709551616"),
        (
            2,
            String::new(),
            "sketchfold: invalid --limit \"18446744073709551616\"\n".to_owned()
        )
    );
}
