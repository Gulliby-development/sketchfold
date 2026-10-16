use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::process::{Command as ProcessCommand, Stdio};

use sketchfold::bundle::{Bundle, Entry, merge_all};
use sketchfold::cli::{Io, run};
use sketchfold::format::{decode, encode};
use sketchfold::frequent::Frequent;
use sketchfold::hist::Hist;
use sketchfold::hll::Hll;
use sketchfold::member::Member;
use sketchfold::sketch::Sketch;

#[derive(Default)]
struct MemoryIo {
    files: RefCell<HashMap<String, Vec<u8>>>,
    stdin: RefCell<Vec<u8>>,
    stdout: RefCell<String>,
    stderr: RefCell<String>,
    reads: RefCell<Vec<String>>,
    write_attempts: RefCell<Vec<(String, Vec<u8>)>>,
    failed_reads: RefCell<HashSet<String>>,
    failed_writes: RefCell<HashSet<String>>,
    stdin_reads: Cell<usize>,
}

impl MemoryIo {
    fn put_file(&self, path: &str, bytes: &[u8]) {
        self.files
            .borrow_mut()
            .insert(path.to_owned(), bytes.to_vec());
    }

    fn fail_read(&self, path: &str) {
        self.failed_reads.borrow_mut().insert(path.to_owned());
    }

    fn fail_write(&self, path: &str) {
        self.failed_writes.borrow_mut().insert(path.to_owned());
    }

    fn set_stdin(&self, bytes: &[u8]) {
        *self.stdin.borrow_mut() = bytes.to_vec();
    }

    fn stdout(&self) -> String {
        self.stdout.borrow().clone()
    }

    fn stderr(&self) -> String {
        self.stderr.borrow().clone()
    }

    fn take_stdout(&self) -> String {
        std::mem::take(&mut *self.stdout.borrow_mut())
    }

    fn take_stderr(&self) -> String {
        std::mem::take(&mut *self.stderr.borrow_mut())
    }
}

impl Io for MemoryIo {
    fn read_file(&self, path: &str) -> Option<Vec<u8>> {
        self.reads.borrow_mut().push(path.to_owned());
        if self.failed_reads.borrow().contains(path) {
            None
        } else {
            self.files.borrow().get(path).cloned()
        }
    }

    fn read_stdin(&self) -> Vec<u8> {
        self.stdin_reads.set(self.stdin_reads.get() + 1);
        self.stdin.borrow().clone()
    }

    fn write_file(&self, path: &str, data: &[u8]) -> bool {
        self.write_attempts
            .borrow_mut()
            .push((path.to_owned(), data.to_vec()));
        if self.failed_writes.borrow().contains(path) {
            false
        } else {
            self.files
                .borrow_mut()
                .insert(path.to_owned(), data.to_vec());
            true
        }
    }

    fn out(&self, text: &str) {
        self.stdout.borrow_mut().push_str(text);
    }

    fn err(&self, text: &str) {
        self.stderr.borrow_mut().push_str(text);
    }
}

fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_owned()).collect()
}

fn run_args(io: &MemoryIo, args: &[&str]) -> i32 {
    run(&argv(args), io)
}

fn entry(name: &str, sketch: Sketch) -> Entry {
    Entry {
        name: name.to_owned(),
        sketch,
    }
}

fn query_bundle() -> Bundle {
    let words = Frequent {
        k: 2,
        n: 12,
        items: vec![(b"plain".to_vec(), 10), (b"line\n".to_vec(), 2)],
    };
    let mut timings = Hist::new(2);
    timings.add(1);
    timings.add(2);
    timings.add(3);
    let mut present = Member::new(6);
    present.add(b"present");

    Bundle {
        entries: vec![
            entry("words", Sketch::Top(words)),
            entry("timings", Sketch::Hist(timings)),
            entry("present", Sketch::Member(present)),
        ],
    }
}

fn put_bundle(io: &MemoryIo, path: &str, bundle: &Bundle) {
    io.put_file(path, encode(bundle).as_bytes());
}

fn assert_cli_error(io: &MemoryIo, args: &[&str], message: &str) {
    io.take_stdout();
    io.take_stderr();
    assert_eq!(run_args(io, args), 2, "args: {args:?}");
    assert_eq!(io.stdout(), "", "args: {args:?}");
    assert_eq!(
        io.stderr(),
        format!("sketchfold: {message}\n"),
        "args: {args:?}"
    );
    assert!(io.write_attempts.borrow().is_empty(), "args: {args:?}");
}

fn assert_failed_merge(left: Bundle, right: Bundle, message: &str) {
    let io = MemoryIo::default();
    put_bundle(&io, "left.sk", &left);
    put_bundle(&io, "right.sk", &right);

    assert_cli_error(
        &io,
        &["merge", "--out", "merged.sk", "left.sk", "right.sk"],
        message,
    );
    assert_eq!(
        *io.reads.borrow(),
        vec!["left.sk".to_owned(), "right.sk".to_owned()]
    );
}

fn top_sketch(k: usize, value: &[u8], count: u64) -> Sketch {
    Sketch::Top(Frequent {
        k,
        n: count,
        items: vec![(value.to_vec(), count)],
    })
}

#[test]
fn help_has_the_exact_seven_lf_terminated_usage_lines() {
    let io = MemoryIo::default();

    assert_eq!(run_args(&io, &["--help"]), 0);
    assert_eq!(
        io.stdout(),
        concat!(
            "usage: sketchfold build --sketch NAME:KIND:FIELD[:PARAM]... [--delim C] --out BUNDLE [FILE...]\n",
            "       sketchfold merge --out BUNDLE BUNDLE BUNDLE...\n",
            "       sketchfold show BUNDLE\n",
            "       sketchfold top [--limit N] BUNDLE NAME\n",
            "       sketchfold quantile BUNDLE NAME Q...\n",
            "       sketchfold has BUNDLE NAME VALUE...\n",
            "       sketchfold verify BUNDLE...\n",
        )
    );
    assert_eq!(io.stderr(), "");
    assert!(io.reads.borrow().is_empty());
    assert!(io.write_attempts.borrow().is_empty());
}

#[test]
fn command_and_option_errors_are_prefixed_and_do_no_io_or_writes() {
    let no_args = MemoryIo::default();
    assert_cli_error(
        &no_args,
        &[],
        "missing command (build, merge, show, top, quantile, has or verify)",
    );
    assert!(no_args.reads.borrow().is_empty());

    let unknown_command = MemoryIo::default();
    assert_cli_error(
        &unknown_command,
        &["inspect"],
        "unknown command \"inspect\"",
    );

    let unknown_option = MemoryIo::default();
    assert_cli_error(
        &unknown_option,
        &["show", "--out=nowhere.sk"],
        "unknown option \"--out=nowhere.sk\"",
    );
    assert!(unknown_option.reads.borrow().is_empty());

    let missing_option_value = MemoryIo::default();
    assert_cli_error(
        &missing_option_value,
        &["build", "--out"],
        "--out needs a value",
    );
    assert!(missing_option_value.reads.borrow().is_empty());

    let invalid_delim = MemoryIo::default();
    assert_cli_error(
        &invalid_delim,
        &[
            "build", "--delim", "ab", "--sketch", "x:top:0", "--out", "x.sk",
        ],
        "invalid --delim \"ab\"",
    );

    let invalid_spec = MemoryIo::default();
    assert_cli_error(
        &invalid_spec,
        &["build", "--sketch", "SPEC", "--out", "x.sk"],
        "invalid --sketch \"SPEC\"",
    );

    let duplicate_spec = MemoryIo::default();
    assert_cli_error(
        &duplicate_spec,
        &[
            "build",
            "--sketch",
            "same:top:0",
            "--sketch",
            "same:hist:0",
            "--out",
            "x.sk",
        ],
        "duplicate sketch name \"same\"",
    );

    let missing_sketch = MemoryIo::default();
    assert_cli_error(
        &missing_sketch,
        &["build", "--out", "x.sk"],
        "missing --sketch",
    );

    let invalid_limit = MemoryIo::default();
    assert_cli_error(
        &invalid_limit,
        &["top", "--limit", "1000001", "bundle.sk", "words"],
        "invalid --limit \"1000001\"",
    );

    let missing_output = MemoryIo::default();
    assert_cli_error(
        &missing_output,
        &["build", "--sketch", "words:top:0"],
        "missing --out",
    );

    let too_few_merge_bundles = MemoryIo::default();
    assert_cli_error(
        &too_few_merge_bundles,
        &["merge", "--out", "merged.sk", "one.sk"],
        "merge needs at least two bundles",
    );

    let missing_bundle = MemoryIo::default();
    assert_cli_error(&missing_bundle, &["show"], "missing BUNDLE");

    let missing_name = MemoryIo::default();
    assert_cli_error(&missing_name, &["top", "bundle.sk"], "missing NAME");

    let missing_quantile = MemoryIo::default();
    assert_cli_error(
        &missing_quantile,
        &["quantile", "bundle.sk", "timings"],
        "missing Q",
    );

    let missing_value = MemoryIo::default();
    assert_cli_error(
        &missing_value,
        &["has", "bundle.sk", "present"],
        "missing VALUE",
    );

    let too_many = MemoryIo::default();
    assert_cli_error(
        &too_many,
        &["show", "one.sk", "two.sk"],
        "too many arguments",
    );

    let duplicate_stdin = MemoryIo::default();
    assert_cli_error(
        &duplicate_stdin,
        &["verify", "-", "-"],
        "- can be given once",
    );

    let invalid_q = MemoryIo::default();
    assert_cli_error(
        &invalid_q,
        &["quantile", "bundle.sk", "timings", "50", "01.2"],
        "invalid quantile \"01.2\"",
    );

    assert!(duplicate_stdin.reads.borrow().is_empty());
    assert_eq!(duplicate_stdin.stdin_reads.get(), 0);
}

#[test]
fn build_reads_ordered_files_and_preserves_line_and_field_rules() {
    let io = MemoryIo::default();
    io.put_file("first.log", "a§/one§1\r\nb§/two§bad\n\n".as_bytes());
    io.put_file("second.log", "c§/one§3\nx§§4\nd§/four".as_bytes());

    assert_eq!(
        run_args(
            &io,
            &[
                "build",
                "--sketch",
                "paths:top:2:5",
                "--sketch",
                "ms:hist:3:4",
                "--delim",
                "§",
                "--out=out.sk",
                "first.log",
                "second.log",
            ],
        ),
        0
    );

    assert_eq!(io.stdout(), "");
    assert_eq!(
        io.stderr(),
        "paths: 4 values, 1 skipped\nms: 3 values, 2 skipped\n"
    );
    assert_eq!(
        *io.reads.borrow(),
        vec!["first.log".to_owned(), "second.log".to_owned()]
    );

    let mut paths = Frequent::new(5);
    paths.add(b"/one");
    paths.add(b"/two");
    paths.add(b"/one");
    paths.add(b"/four");
    let mut milliseconds = Hist::new(4);
    milliseconds.add(1);
    milliseconds.add(3);
    milliseconds.add(4);
    let expected = Bundle {
        entries: vec![
            entry("paths", Sketch::Top(paths)),
            entry("ms", Sketch::Hist(milliseconds)),
        ],
    };
    assert_eq!(
        io.files.borrow().get("out.sk"),
        Some(&encode(&expected).into_bytes())
    );
    let decoded = decode(io.files.borrow().get("out.sk").unwrap()).unwrap();
    assert_eq!(decoded, expected);
}

#[test]
fn build_without_paths_reads_stdin_once_and_stdout_contains_only_the_bundle() {
    let io = MemoryIo::default();
    io.set_stdin(b"alpha\n");
    let mut expected_top = Frequent::new(16);
    expected_top.add(b"alpha");
    let expected_bundle = Bundle {
        entries: vec![entry("word", Sketch::Top(expected_top))],
    };

    assert_eq!(
        run_args(&io, &["build", "--sketch", "word:top:0", "--out=-",],),
        0
    );

    assert_eq!(io.stdin_reads.get(), 1);
    assert_eq!(io.stdout(), encode(&expected_bundle));
    assert_eq!(io.stderr(), "word: 1 value, 0 skipped\n");
    assert!(io.write_attempts.borrow().is_empty());
}

#[test]
fn build_accepts_stdout_and_one_stdin_path_together() {
    let io = MemoryIo::default();
    io.set_stdin(b"one\n");

    assert_eq!(
        run_args(&io, &["build", "--sketch", "word:top:0", "--out", "-", "-"],),
        0
    );

    assert_eq!(io.stdin_reads.get(), 1);
    assert_eq!(
        io.stdout(),
        concat!(
            "sketchfold 1\n",
            "sketch word top k=16 n=1\n",
            "item 1 6f6e65\n",
            "end e6444471\n",
        )
    );
    assert_eq!(io.stderr(), "word: 1 value, 0 skipped\n");
}

#[test]
fn build_reports_read_failure_without_publishing_bundle_or_statistics() {
    let io = MemoryIo::default();
    io.put_file("good.log", b"ready\n");
    io.fail_read("logs\\missing.log");

    assert_eq!(
        run_args(
            &io,
            &[
                "build",
                "--sketch",
                "word:top:0",
                "--out",
                "out.sk",
                "good.log",
                "logs\\missing.log",
            ],
        ),
        2
    );
    assert_eq!(io.stdout(), "");
    assert_eq!(
        io.stderr(),
        "sketchfold: logs/missing.log: cannot read the file\n"
    );
    assert_eq!(
        *io.reads.borrow(),
        vec!["good.log".to_owned(), "logs\\missing.log".to_owned()]
    );
    assert!(io.write_attempts.borrow().is_empty());
    assert!(!io.files.borrow().contains_key("out.sk"));
}

#[test]
fn build_write_failure_has_no_success_statistics_and_does_not_mutate_storage() {
    let io = MemoryIo::default();
    io.put_file("input.log", b"ready\n");
    io.put_file("output\\bundle.sk", b"unchanged");
    io.fail_write("output\\bundle.sk");

    assert_eq!(
        run_args(
            &io,
            &[
                "build",
                "--sketch",
                "word:top:0",
                "--out",
                "output\\bundle.sk",
                "input.log",
            ],
        ),
        2
    );
    assert_eq!(io.stdout(), "");
    assert_eq!(
        io.stderr(),
        "sketchfold: output/bundle.sk: cannot write the file\n"
    );
    assert_eq!(
        *io.files.borrow().get("output\\bundle.sk").unwrap(),
        b"unchanged"
    );
    assert_eq!(io.write_attempts.borrow().len(), 1);
}

#[test]
fn merge_keeps_first_seen_order_and_writes_only_after_success() {
    let io = MemoryIo::default();

    let mut first_values = Frequent::new(3);
    first_values.add(b"shared");
    let left = Bundle {
        entries: vec![
            entry("first", top_sketch(2, b"one", 1)),
            entry("shared", Sketch::Top(first_values)),
        ],
    };

    let mut shared_values = Frequent::new(3);
    shared_values.add(b"shared");
    shared_values.add(b"shared");
    let right = Bundle {
        entries: vec![
            entry("shared", Sketch::Top(shared_values)),
            entry("later", Sketch::Member(Member::new(6))),
        ],
    };
    put_bundle(&io, "left.sk", &left);
    put_bundle(&io, "right.sk", &right);
    let expected = merge_all(vec![left, right]).unwrap();

    assert_eq!(
        run_args(&io, &["merge", "--out", "merged.sk", "left.sk", "right.sk"],),
        0
    );
    assert_eq!(io.stdout(), "");
    assert_eq!(io.stderr(), "merged 2 bundles, 3 sketches\n");
    assert_eq!(
        decode(io.files.borrow().get("merged.sk").unwrap()).unwrap(),
        expected
    );
    assert_eq!(
        io.write_attempts.borrow().as_slice(),
        [(String::from("merged.sk"), encode(&expected).into_bytes())]
    );
}

#[test]
fn merge_uses_singular_sketch_statistics_for_one_retained_name() {
    let io = MemoryIo::default();
    let first = Bundle {
        entries: vec![entry("word", Sketch::Top(Frequent::new(2)))],
    };
    let second = Bundle {
        entries: vec![entry("word", Sketch::Top(Frequent::new(2)))],
    };
    put_bundle(&io, "a.sk", &first);
    put_bundle(&io, "b.sk", &second);

    assert_eq!(
        run_args(&io, &["merge", "--out", "out.sk", "a.sk", "b.sk"]),
        0
    );
    assert_eq!(io.stderr(), "merged 2 bundles, 1 sketch\n");
}

#[test]
fn merge_read_or_decode_failure_never_publishes_partial_output() {
    let io = MemoryIo::default();
    put_bundle(&io, "valid.sk", &query_bundle());
    io.put_file("bad.sk", b"not a bundle\n");

    assert_eq!(
        run_args(&io, &["merge", "--out", "out.sk", "valid.sk", "bad.sk"],),
        2
    );
    assert_eq!(io.stdout(), "");
    assert_eq!(
        io.stderr(),
        "sketchfold: bad.sk:1: not a sketchfold bundle\n"
    );
    assert!(io.write_attempts.borrow().is_empty());
    assert!(!io.files.borrow().contains_key("out.sk"));

    let missing = MemoryIo::default();
    put_bundle(&missing, "valid.sk", &query_bundle());
    missing.fail_read("later.sk");
    assert_eq!(
        run_args(
            &missing,
            &["merge", "--out", "out.sk", "valid.sk", "later.sk"],
        ),
        2
    );
    assert_eq!(
        missing.stderr(),
        "sketchfold: later.sk: cannot read the file\n"
    );
    assert!(missing.write_attempts.borrow().is_empty());
}

#[test]
fn merge_kind_conflict_reports_the_left_and_right_kinds() {
    assert_failed_merge(
        Bundle {
            entries: vec![entry("same", Sketch::Distinct(Hll::new(12)))],
        },
        Bundle {
            entries: vec![entry("same", Sketch::Top(Frequent::new(16)))],
        },
        "same: cannot merge distinct and top",
    );
}

#[test]
fn merge_parameter_conflicts_report_each_upstream_parameter_name_and_value() {
    assert_failed_merge(
        Bundle {
            entries: vec![entry("d", Sketch::Distinct(Hll::new(12)))],
        },
        Bundle {
            entries: vec![entry("d", Sketch::Distinct(Hll::new(10)))],
        },
        "d: precision differs (12 vs 10)",
    );

    assert_failed_merge(
        Bundle {
            entries: vec![entry("t", Sketch::Top(Frequent::new(16)))],
        },
        Bundle {
            entries: vec![entry("t", Sketch::Top(Frequent::new(8)))],
        },
        "t: capacity differs (16 vs 8)",
    );

    assert_failed_merge(
        Bundle {
            entries: vec![entry("h", Sketch::Hist(Hist::new(4)))],
        },
        Bundle {
            entries: vec![entry("h", Sketch::Hist(Hist::new(5)))],
        },
        "h: bits differs (4 vs 5)",
    );

    assert_failed_merge(
        Bundle {
            entries: vec![entry("m", Sketch::Member(Member::new(14)))],
        },
        Bundle {
            entries: vec![entry("m", Sketch::Member(Member::new(10)))],
        },
        "m: size differs (14 vs 10)",
    );
}

#[test]
fn show_renders_order_padding_details_and_empty_histograms_exactly() {
    let mut values = Hist::new(2);
    values.add(1);
    values.add(2);
    values.add(3);
    let bundle = Bundle {
        entries: vec![
            entry("words", Sketch::Top(Frequent::new(2))),
            entry("timings", Sketch::Hist(values)),
            entry("empty", Sketch::Hist(Hist::new(2))),
            entry("present", Sketch::Member(Member::new(6))),
        ],
    };
    let empty_bundle = Bundle {
        entries: vec![entry("empty", Sketch::Hist(Hist::new(2)))],
    };
    let io = MemoryIo::default();
    put_bundle(&io, "bundle.sk", &bundle);
    put_bundle(&io, "empty.sk", &empty_bundle);

    assert_eq!(run_args(&io, &["show", "bundle.sk"]), 0);
    assert_eq!(
        io.stdout(),
        concat!(
            "words    top       k=2  n=0  0 counters, slack 0\n",
            "timings  hist      s=2  n=3  min 1, max 3, p50 2, p90 3, p99 3\n",
            "empty    hist      s=2  n=0  empty\n",
            "present  member    b=6  n=0  0 of 64 bits set\n",
        )
    );
    assert_eq!(io.stderr(), "");

    io.take_stdout();
    assert_eq!(run_args(&io, &["show", "empty.sk"]), 0);
    assert_eq!(io.stdout(), "empty  hist      s=2  n=0  empty\n");

    io.take_stdout();
    let empty_bundle = Bundle::default();
    put_bundle(&io, "no-sketches.sk", &empty_bundle);
    assert_eq!(run_args(&io, &["show", "no-sketches.sk"]), 0);
    assert_eq!(io.stdout(), "");
}

#[test]
fn top_renders_escaped_values_with_aligned_counts_and_honors_limit() {
    let io = MemoryIo::default();
    put_bundle(&io, "queries.sk", &query_bundle());

    assert_eq!(
        run_args(&io, &["top", "--limit=1", "queries.sk", "words"]),
        0
    );
    assert_eq!(io.stdout(), "words: n=12, slack 0\n10  plain\n");
    assert_eq!(io.stderr(), "");

    io.take_stdout();
    assert_eq!(run_args(&io, &["top", "queries.sk", "words"]), 0);
    assert_eq!(
        io.stdout(),
        "words: n=12, slack 0\n10  plain\n 2  line\\x0a\n"
    );
    assert_eq!(io.write_attempts.borrow().len(), 0);
}

#[test]
fn query_errors_cover_absent_names_wrong_kinds_and_empty_histograms() {
    let io = MemoryIo::default();
    let mut empty = Bundle::default();
    empty
        .entries
        .push(entry("empty", Sketch::Hist(Hist::new(1))));
    let bundle = query_bundle();
    put_bundle(&io, "queries.sk", &bundle);
    put_bundle(&io, "empty.sk", &empty);

    assert_cli_error(
        &io,
        &["top", "queries.sk", "absent"],
        "absent: no such sketch",
    );
    assert_cli_error(
        &io,
        &["top", "queries.sk", "timings"],
        "timings: not a top sketch",
    );
    assert_cli_error(
        &io,
        &["quantile", "queries.sk", "words", "50"],
        "words: not a hist sketch",
    );
    assert_cli_error(
        &io,
        &["has", "queries.sk", "timings", "1"],
        "timings: not a member sketch",
    );
    assert_cli_error(
        &io,
        &["quantile", "empty.sk", "empty", "50"],
        "empty: sketch is empty",
    );
}

#[test]
fn quantile_queries_preserve_spelling_and_clamp_zero_and_one_hundred() {
    let io = MemoryIo::default();
    put_bundle(&io, "queries.sk", &query_bundle());

    assert_eq!(
        run_args(
            &io,
            &[
                "quantile",
                "queries.sk",
                "timings",
                "0",
                "50.000",
                "99.9",
                "100",
            ],
        ),
        0
    );
    assert_eq!(io.stdout(), "p0 1\np50.000 2\np99.9 3\np100 3\n");
    assert_eq!(io.stderr(), "");
    assert!(io.write_attempts.borrow().is_empty());
}

#[test]
fn membership_queries_report_every_value_and_fail_if_any_is_absent() {
    let io = MemoryIo::default();
    put_bundle(&io, "queries.sk", &query_bundle());
    let decoded = decode(io.files.borrow().get("queries.sk").unwrap()).unwrap();
    let Sketch::Member(member) = decoded.get("present").unwrap() else {
        panic!("fixture member sketch");
    };
    assert!(member.has(b"present"));
    assert!(!member.has(b"absent"));

    assert_eq!(
        run_args(
            &io,
            &[
                "has",
                "queries.sk",
                "present",
                "present",
                "present",
                "absent"
            ],
        ),
        1
    );
    assert_eq!(io.stdout(), "present: maybe\npresent: maybe\nabsent: no\n");
    assert_eq!(io.stderr(), "");
    assert!(io.write_attempts.borrow().is_empty());
}

#[test]
fn verify_continues_after_bad_and_unreadable_bundles() {
    let io = MemoryIo::default();
    put_bundle(&io, "good.sk", &query_bundle());
    io.put_file("broken.sk", b"invalid\n");
    io.fail_read("missing.sk");

    assert_eq!(
        run_args(&io, &["verify", "good.sk", "broken.sk", "missing.sk"]),
        1
    );
    assert_eq!(
        io.stdout(),
        concat!(
            "good.sk: ok (3 sketches)\n",
            "broken.sk:1: not a sketchfold bundle\n",
            "missing.sk: cannot read the file\n",
        )
    );
    assert_eq!(io.stderr(), "");
    assert!(io.write_attempts.borrow().is_empty());
    assert_eq!(
        *io.reads.borrow(),
        vec![
            "good.sk".to_owned(),
            "broken.sk".to_owned(),
            "missing.sk".to_owned()
        ]
    );
}

#[test]
fn verify_reads_stdin_and_displays_it_as_stdin() {
    let io = MemoryIo::default();
    io.set_stdin(encode(&query_bundle()).as_bytes());

    assert_eq!(run_args(&io, &["verify", "-"]), 0);
    assert_eq!(io.stdout(), "<stdin>: ok (3 sketches)\n");
    assert_eq!(io.stderr(), "");
    assert_eq!(io.stdin_reads.get(), 1);
}

#[test]
fn failed_read_paths_are_normalized_for_display_but_not_for_lookup() {
    let io = MemoryIo::default();
    io.fail_read("folder\\missing.sk");

    assert_eq!(run_args(&io, &["show", "folder\\missing.sk"]), 2);
    assert_eq!(
        io.stderr(),
        "sketchfold: folder/missing.sk: cannot read the file\n"
    );
    assert_eq!(*io.reads.borrow(), vec!["folder\\missing.sk".to_owned()]);
    assert!(io.write_attempts.borrow().is_empty());
}

#[test]
fn worked_access_log_commands_match_exact_outputs_and_bundle_checksum() {
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
    let first_half = concat!(
        "10.0.0.1 /index.html 200 12\n",
        "10.0.0.2 /index.html 200 15\n",
        "10.0.0.1 /about 200 40\n",
        "10.0.0.3 /index.html 304 3\n",
        "10.0.0.2 /api/items 500 950\n",
    );
    let second_half = concat!(
        "10.0.0.4 /index.html 200 18\n",
        "10.0.0.1 /api/items 200 120\n",
        "10.0.0.5 /about 404 7\n",
        "10.0.0.3 /index.html 200 14\n",
        "10.0.0.2 /api/items 200 130\n",
    );
    let io = MemoryIo::default();
    io.put_file("access.log", access_log.as_bytes());

    let build_args = [
        "build",
        "--sketch",
        "ips:distinct:1:4",
        "--sketch",
        "paths:top:2:2",
        "--sketch",
        "ms:hist:4:4",
        "--sketch",
        "seen:member:2:6",
        "--out",
        "access.sk",
        "access.log",
    ];
    assert_eq!(run(&argv(&build_args), &io), 0);
    assert_eq!(
        io.stderr(),
        concat!(
            "ips: 10 values, 0 skipped\n",
            "paths: 10 values, 0 skipped\n",
            "ms: 10 values, 0 skipped\n",
            "seen: 10 values, 0 skipped\n",
        )
    );
    io.take_stderr();
    assert_eq!(io.stdout(), "");

    assert_eq!(run_args(&io, &["show", "access.sk"]), 0);
    let expected_show = concat!(
        "ips    distinct  p=4  n=10  about 6 distinct, error 26.0%\n",
        "paths  top       k=2  n=10  2 counters, slack 2\n",
        "ms     hist      s=4  n=10  min 3, max 950, p50 15, p90 135, p99 950\n",
        "seen   member    b=6  n=10  12 of 64 bits set\n",
    );
    assert_eq!(io.stdout(), expected_show);
    assert_eq!(io.stderr(), "");

    io.take_stdout();
    assert_eq!(run_args(&io, &["top", "access.sk", "paths"]), 0);
    assert_eq!(
        io.stdout(),
        "paths: n=10, slack 2\n3  /index.html\n1  /api/items\n"
    );

    io.take_stdout();
    assert_eq!(
        run_args(
            &io,
            &[
                "quantile",
                "access.sk",
                "ms",
                "50",
                "90",
                "99.9",
                "0",
                "100"
            ],
        ),
        0
    );
    assert_eq!(io.stdout(), "p50 15\np90 135\np99.9 950\np0 3\np100 950\n");

    io.take_stdout();
    assert_eq!(
        run_args(&io, &["has", "access.sk", "seen", "/about", "/nope"]),
        1
    );
    assert_eq!(io.stdout(), "/about: maybe\n/nope: no\n");
    io.take_stdout();

    io.put_file("access.log.part", first_half.as_bytes());
    io.put_file("access.log.rest", second_half.as_bytes());
    let split_first = [
        "build",
        "--sketch",
        "ips:distinct:1:4",
        "--sketch",
        "paths:top:2:2",
        "--sketch",
        "ms:hist:4:4",
        "--sketch",
        "seen:member:2:6",
        "--out",
        "a.sk",
        "access.log.part",
    ];
    let split_second = [
        "build",
        "--sketch",
        "ips:distinct:1:4",
        "--sketch",
        "paths:top:2:2",
        "--sketch",
        "ms:hist:4:4",
        "--sketch",
        "seen:member:2:6",
        "--out",
        "b.sk",
        "access.log.rest",
    ];
    assert_eq!(run(&argv(&split_first), &io), 0);
    assert_eq!(
        io.take_stderr(),
        "ips: 5 values, 0 skipped\npaths: 5 values, 0 skipped\nms: 5 values, 0 skipped\nseen: 5 values, 0 skipped\n"
    );
    assert_eq!(run(&argv(&split_second), &io), 0);
    assert_eq!(
        io.take_stderr(),
        "ips: 5 values, 0 skipped\npaths: 5 values, 0 skipped\nms: 5 values, 0 skipped\nseen: 5 values, 0 skipped\n"
    );
    assert_eq!(
        run_args(&io, &["merge", "--out", "all.sk", "a.sk", "b.sk"],),
        0
    );
    assert_eq!(io.stderr(), "merged 2 bundles, 4 sketches\n");
    assert_eq!(io.stdout(), "");
    assert_eq!(run_args(&io, &["show", "all.sk"]), 0);
    assert_eq!(io.stdout(), expected_show);

    let ips_only = [
        "build",
        "--sketch",
        "ips:distinct:1:4",
        "--out",
        "-",
        "access.log",
    ];
    io.take_stdout();
    io.take_stderr();
    assert_eq!(run(&argv(&ips_only), &io), 0);
    assert_eq!(
        io.stdout(),
        concat!(
            "sketchfold 1\n",
            "sketch ips distinct p=4 n=10\n",
            "regs 01000000000000010100000200030000\n",
            "end c657f82f\n",
        )
    );
    assert_eq!(io.stderr(), "ips: 10 values, 0 skipped\n");
    io.take_stdout();
    io.take_stderr();

    io.put_file(
        "bad.sk",
        concat!(
            "sketchfold 1\n",
            "sketch ips distinct p=4 n=11\n",
            "regs 01000000000000010100000200030000\n",
            "end c657f82f\n",
        )
        .as_bytes(),
    );
    assert_eq!(run_args(&io, &["verify", "bad.sk", "access.sk"]), 1);
    assert_eq!(
        io.stdout(),
        "bad.sk:4: checksum mismatch\naccess.sk: ok (4 sketches)\n"
    );
    assert_eq!(io.stderr(), "");
}

#[test]
fn failed_merge_output_does_not_emit_success_summary_or_change_the_target() {
    let io = MemoryIo::default();
    let left = Bundle {
        entries: vec![entry("a", Sketch::Top(Frequent::new(2)))],
    };
    let right = Bundle {
        entries: vec![entry("b", Sketch::Top(Frequent::new(2)))],
    };
    put_bundle(&io, "a.sk", &left);
    put_bundle(&io, "b.sk", &right);
    io.put_file("out.sk", b"original");
    io.fail_write("out.sk");

    assert_eq!(
        run_args(&io, &["merge", "--out", "out.sk", "a.sk", "b.sk"]),
        2
    );
    assert_eq!(io.stdout(), "");
    assert_eq!(io.stderr(), "sketchfold: out.sk: cannot write the file\n");
    assert_eq!(*io.files.borrow().get("out.sk").unwrap(), b"original");
    assert_eq!(io.write_attempts.borrow().len(), 1);
}

#[test]
fn empty_bundle_reports_empty_show_and_singular_verify_counts() {
    let io = MemoryIo::default();
    put_bundle(&io, "empty.sk", &Bundle::default());

    assert_eq!(run_args(&io, &["show", "empty.sk"]), 0);
    assert_eq!(io.stdout(), "");
    assert_eq!(run_args(&io, &["verify", "empty.sk"]), 0);
    assert_eq!(io.stdout(), "empty.sk: ok (0 sketches)\n");
}

#[test]
fn the_real_binary_preserves_malformed_utf8_stdin_as_value_bytes() {
    let mut child = ProcessCommand::new(env!("CARGO_BIN_EXE_sketchfold"))
        .args(["build", "--sketch", "w:top:0", "--out", "-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sketchfold process for the required raw-byte test");

    child
        .stdin
        .as_mut()
        .expect("stdin is piped")
        .write_all(&[0x61, 0xff, 0x62, 0x0a])
        .expect("write the exact malformed UTF-8 input bytes");
    drop(child.stdin.take());
    let output = child
        .wait_with_output()
        .expect("collect sketchfold process output");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        output.stdout,
        b"sketchfold 1\nsketch w top k=16 n=1\nitem 1 61ff62\nend 1267c1e1\n"
    );
    assert_eq!(output.stderr, b"w: 1 value, 0 skipped\n");
}
