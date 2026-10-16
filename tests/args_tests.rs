use sketchfold::args::{Command, parse};
use sketchfold::error::CliError;
use sketchfold::spec::{Kind, Spec};

fn arguments(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

fn parse_words(words: &[&str]) -> Result<Command, CliError> {
    parse(&arguments(words))
}

fn assert_error(words: &[&str], expected: &str) {
    assert_eq!(
        parse_words(words),
        Err(CliError(expected.to_owned())),
        "unexpected result for {words:?}"
    );
}

#[test]
fn empty_arguments_and_unknown_commands_have_exact_errors() {
    assert_error(
        &[],
        "missing command (build, merge, show, top, quantile, has or verify)",
    );
    assert_error(&["publish"], "unknown command \"publish\"");
}

#[test]
fn only_a_sole_help_argument_is_special() {
    assert_eq!(parse_words(&["--help"]), Ok(Command::Help));
    assert_error(&["build", "--help"], "unknown option \"--help\"");
    assert_error(&["--help", "show"], "unknown command \"--help\"");
}

#[test]
fn build_requires_a_sketch_before_it_requires_an_output() {
    assert_error(&["build", "--out", "bundle"], "missing --sketch");
    assert_error(
        &["build", "--sketch=SPEC", "--out=bundle"],
        "invalid --sketch \"SPEC\"",
    );
    assert_error(&["build", "--sketch", "values:top:0"], "missing --out");
}

#[test]
fn build_defaults_to_space_and_stdin_when_no_input_files_are_given() {
    assert_eq!(
        parse_words(&["build", "--sketch", "values:top:0", "--out", "bundle"]),
        Ok(Command::Build {
            specs: vec![Spec {
                name: "values".to_owned(),
                kind: Kind::Top,
                field: 0,
                param: 16,
            }],
            delim: b" ".to_vec(),
            out: "bundle".to_owned(),
            inputs: vec![],
        })
    );
}

#[test]
fn build_collects_ordered_specs_and_interspersed_positionals() {
    assert_eq!(
        parse_words(&[
            "build",
            "first.log",
            "--sketch=ips:distinct:1:4",
            "second.log",
            "--delim=§",
            "--out=old.sk",
            "--sketch",
            "latency:hist:3:8",
            "--out",
            "new.sk",
            "third.log",
        ]),
        Ok(Command::Build {
            specs: vec![
                Spec {
                    name: "ips".to_owned(),
                    kind: Kind::Distinct,
                    field: 1,
                    param: 4,
                },
                Spec {
                    name: "latency".to_owned(),
                    kind: Kind::Hist,
                    field: 3,
                    param: 8,
                },
            ],
            delim: "§".as_bytes().to_vec(),
            out: "new.sk".to_owned(),
            inputs: vec![
                "first.log".to_owned(),
                "second.log".to_owned(),
                "third.log".to_owned(),
            ],
        })
    );
}

#[test]
fn build_accepts_empty_equals_suffix_for_output_and_last_singletons_win() {
    assert_eq!(
        parse_words(&[
            "build",
            "--sketch=a:member:99:20",
            "--delim=x",
            "--delim=::",
            "--out=first",
            "--out=",
        ]),
        Err(CliError("invalid --delim \"::\"".to_owned()))
    );
    assert_eq!(
        parse_words(&[
            "build",
            "--sketch=a:member:99:20",
            "--delim=x",
            "--delim=§",
            "--out=first",
            "--out=",
        ]),
        Ok(Command::Build {
            specs: vec![Spec {
                name: "a".to_owned(),
                kind: Kind::Member,
                field: 99,
                param: 20,
            }],
            delim: "§".as_bytes().to_vec(),
            out: String::new(),
            inputs: vec![],
        })
    );
}

#[test]
fn build_consumes_a_dash_prefixed_option_value_and_keeps_output_independent_of_input_dash() {
    assert_eq!(
        parse_words(&[
            "build",
            "--sketch",
            "values:top:0:1",
            "--out",
            "--destination",
            "-",
        ]),
        Ok(Command::Build {
            specs: vec![Spec {
                name: "values".to_owned(),
                kind: Kind::Top,
                field: 0,
                param: 1,
            }],
            delim: b" ".to_vec(),
            out: "--destination".to_owned(),
            inputs: vec!["-".to_owned()],
        })
    );
    assert_eq!(
        parse_words(&["build", "--sketch", "values:top:0", "--out=-", "-",]),
        Ok(Command::Build {
            specs: vec![Spec {
                name: "values".to_owned(),
                kind: Kind::Top,
                field: 0,
                param: 16,
            }],
            delim: b" ".to_vec(),
            out: "-".to_owned(),
            inputs: vec!["-".to_owned()],
        })
    );
}

#[test]
fn repeated_sketches_keep_order_and_use_upstream_defaults() {
    assert_eq!(
        parse_words(&[
            "build",
            "--out",
            "bundle",
            "--sketch",
            "one:distinct:0",
            "--sketch=two:top:1",
            "--sketch",
            "three:hist:2",
            "--sketch",
            "four:member:3",
        ]),
        Ok(Command::Build {
            specs: vec![
                Spec {
                    name: "one".to_owned(),
                    kind: Kind::Distinct,
                    field: 0,
                    param: 12,
                },
                Spec {
                    name: "two".to_owned(),
                    kind: Kind::Top,
                    field: 1,
                    param: 16,
                },
                Spec {
                    name: "three".to_owned(),
                    kind: Kind::Hist,
                    field: 2,
                    param: 4,
                },
                Spec {
                    name: "four".to_owned(),
                    kind: Kind::Member,
                    field: 3,
                    param: 14,
                },
            ],
            delim: b" ".to_vec(),
            out: "bundle".to_owned(),
            inputs: vec![],
        })
    );
}

#[test]
fn option_scanning_errors_are_left_to_right_and_precede_semantic_validation() {
    assert_error(
        &["build", "--delim=", "--sketch", "bad", "--unsupported"],
        "unknown option \"--unsupported\"",
    );
    assert_error(&["build", "--out"], "--out needs a value");
    assert_error(&["build", "--sketch"], "--sketch needs a value");
    assert_error(&["build", "--delim"], "--delim needs a value");
    assert_error(&["merge", "--out"], "--out needs a value");
    assert_error(&["top", "--limit"], "--limit needs a value");
}

#[test]
fn unsupported_options_are_reported_as_typed_including_equals_and_short_forms() {
    assert_error(&["build", "--limit", "2"], "unknown option \"--limit\"");
    assert_error(&["merge", "--delim=x"], "unknown option \"--delim=x\"");
    assert_error(&["show", "--out=x"], "unknown option \"--out=x\"");
    assert_error(&["top", "--out", "x"], "unknown option \"--out\"");
    assert_error(&["quantile", "--limit=4"], "unknown option \"--limit=4\"");
    assert_error(&["has", "-o"], "unknown option \"-o\"");
    assert_error(&["verify", "--"], "unknown option \"--\"");
    assert_error(&["show", "bundle", "--ARG"], "unknown option \"--ARG\"");
}

#[test]
fn empty_equals_suffix_is_a_value_not_a_missing_value() {
    assert_error(
        &["build", "--sketch=", "--out", "bundle"],
        "invalid --sketch \"\"",
    );
    assert_error(
        &[
            "build", "--sketch", "a:top:0", "--delim=", "--out", "bundle",
        ],
        "invalid --delim \"\"",
    );
    assert_eq!(
        parse_words(&["build", "--sketch=a:top:0", "--out="]),
        Ok(Command::Build {
            specs: vec![Spec {
                name: "a".to_owned(),
                kind: Kind::Top,
                field: 0,
                param: 16,
            }],
            delim: b" ".to_vec(),
            out: String::new(),
            inputs: vec![],
        })
    );
}

#[test]
fn one_unicode_code_point_delimiter_is_accepted_but_empty_or_multiple_are_rejected() {
    assert_eq!(
        parse_words(&["build", "--sketch=a:top:0", "--delim=界", "--out=bundle",]),
        Ok(Command::Build {
            specs: vec![Spec {
                name: "a".to_owned(),
                kind: Kind::Top,
                field: 0,
                param: 16,
            }],
            delim: "界".as_bytes().to_vec(),
            out: "bundle".to_owned(),
            inputs: vec![],
        })
    );
    assert_error(
        &["build", "--sketch=a:top:0", "--delim=", "--out=bundle"],
        "invalid --delim \"\"",
    );
    assert_error(
        &["build", "--sketch=a:top:0", "--delim=ab", "--out=bundle"],
        "invalid --delim \"ab\"",
    );
}

#[test]
fn delimiter_validation_precedes_spec_validation() {
    assert_error(
        &["build", "--sketch=bad", "--delim=ab", "--out=bundle"],
        "invalid --delim \"ab\"",
    );
}

#[test]
fn all_sketches_are_parsed_before_duplicate_names_are_checked() {
    assert_error(
        &[
            "build",
            "--sketch=same:top:0",
            "--sketch=same:hist:1",
            "--sketch=broken",
            "--out=bundle",
        ],
        "invalid --sketch \"broken\"",
    );
}

#[test]
fn invalid_sketch_diagnostic_uses_the_first_bad_spec() {
    assert_error(
        &[
            "build",
            "--sketch=broken",
            "--sketch=also-broken",
            "--out=bundle",
        ],
        "invalid --sketch \"broken\"",
    );
}

#[test]
fn duplicate_sketch_names_are_rejected_after_successful_spec_parsing() {
    assert_error(
        &[
            "build",
            "--sketch=same:top:0",
            "--sketch=same:member:1",
            "--out=bundle",
        ],
        "duplicate sketch name \"same\"",
    );
}

#[test]
fn final_limit_value_is_validated_and_inclusive_endpoints_are_accepted() {
    assert_eq!(
        parse_words(&["top", "--limit=1", "--limit=1000000", "bundle", "words"]),
        Ok(Command::Top {
            bundle: "bundle".to_owned(),
            name: "words".to_owned(),
            limit: Some(1_000_000),
        })
    );
    assert_error(
        &["top", "--limit=1", "--limit=0", "bundle", "words"],
        "invalid --limit \"0\"",
    );
    assert_eq!(
        parse_words(&["top", "--limit=0", "--limit=1", "bundle", "words"]),
        Ok(Command::Top {
            bundle: "bundle".to_owned(),
            name: "words".to_owned(),
            limit: Some(1),
        })
    );
}

#[test]
fn invalid_limits_reject_overflow_noncanonical_and_out_of_range_values() {
    assert_error(
        &["top", "--limit=0", "bundle", "words"],
        "invalid --limit \"0\"",
    );
    assert_error(
        &["top", "--limit=1000001", "bundle", "words"],
        "invalid --limit \"1000001\"",
    );
    assert_error(
        &["top", "--limit=01", "bundle", "words"],
        "invalid --limit \"01\"",
    );
    assert_error(
        &["top", "--limit=18446744073709551616", "bundle", "words"],
        "invalid --limit \"18446744073709551616\"",
    );
    assert_error(
        &["top", "--limit=-1", "bundle", "words"],
        "invalid --limit \"-1\"",
    );
}

#[test]
fn limit_validation_precedes_top_arity_checks() {
    assert_error(&["top", "--limit=0"], "invalid --limit \"0\"");
    assert_error(&["top", "--limit", "-1"], "invalid --limit \"-1\"");
}

#[test]
fn command_specific_missing_positional_diagnostics_are_exact() {
    assert_error(
        &["merge", "--out=bundle"],
        "merge needs at least two bundles",
    );
    assert_error(&["merge", "left", "right"], "missing --out");
    assert_error(&["show"], "missing BUNDLE");
    assert_error(&["top"], "missing BUNDLE");
    assert_error(&["top", "bundle"], "missing NAME");
    assert_error(&["quantile"], "missing BUNDLE");
    assert_error(&["quantile", "bundle"], "missing NAME");
    assert_error(&["quantile", "bundle", "latency"], "missing Q");
    assert_error(&["has"], "missing BUNDLE");
    assert_error(&["has", "bundle"], "missing NAME");
    assert_error(&["has", "bundle", "seen"], "missing VALUE");
    assert_error(&["verify"], "missing BUNDLE");
}

#[test]
fn missing_output_and_merge_arity_precede_later_path_validation() {
    assert_error(&["merge", "-"], "missing --out");
    assert_error(
        &["merge", "--out=result", "-"],
        "merge needs at least two bundles",
    );
}

#[test]
fn extra_fixed_arity_arguments_are_reported_before_dash_uniqueness() {
    assert_error(&["show", "-", "-"], "too many arguments");
    assert_error(&["top", "bundle", "words", "extra"], "too many arguments");
    assert_error(
        &["build", "--sketch=a:top:0", "--out=bundle", "-", "-"],
        "- can be given once",
    );
}

#[test]
fn duplicate_stdin_paths_are_rejected_but_output_dash_is_independent() {
    assert_error(
        &["build", "--sketch=a:top:0", "--out=bundle", "-", "-"],
        "- can be given once",
    );
    assert_error(&["verify", "-", "-"], "- can be given once");
    assert_eq!(
        parse_words(&["merge", "--out=-", "-", "right.sk"]),
        Ok(Command::Merge {
            out: "-".to_owned(),
            bundles: vec!["-".to_owned(), "right.sk".to_owned()],
        })
    );
}

#[test]
fn top_name_dash_is_not_a_second_input_path() {
    assert_eq!(
        parse_words(&["top", "-", "-"]),
        Ok(Command::Top {
            bundle: "-".to_owned(),
            name: "-".to_owned(),
            limit: None,
        })
    );
}

#[test]
fn quantile_name_dash_is_not_a_second_input_path() {
    assert_eq!(
        parse_words(&["quantile", "-", "-", "0"]),
        Ok(Command::Quantile {
            bundle: "-".to_owned(),
            name: "-".to_owned(),
            quantiles: vec![("0".to_owned(), 0)],
        })
    );
}

#[test]
fn has_name_and_value_dashes_are_not_input_paths() {
    assert_eq!(
        parse_words(&["has", "-", "-", "-"]),
        Ok(Command::Has {
            bundle: "-".to_owned(),
            name: "-".to_owned(),
            values: vec!["-".to_owned()],
        })
    );
}

#[test]
fn query_parsing_keeps_typed_quantiles_and_converts_thousandths_exactly() {
    assert_eq!(
        parse_words(&[
            "quantile", "bundle", "latency", "0", "0.001", "50", "50.000", "99.9", "100",
            "100.000",
        ]),
        Ok(Command::Quantile {
            bundle: "bundle".to_owned(),
            name: "latency".to_owned(),
            quantiles: vec![
                ("0".to_owned(), 0),
                ("0.001".to_owned(), 1),
                ("50".to_owned(), 50_000),
                ("50.000".to_owned(), 50_000),
                ("99.9".to_owned(), 99_900),
                ("100".to_owned(), 100_000),
                ("100.000".to_owned(), 100_000),
            ],
        })
    );
}

#[test]
fn invalid_quantile_forms_report_the_first_bad_spelling() {
    assert_error(&["quantile", "b", "h", "00"], "invalid quantile \"00\"");
    assert_error(&["quantile", "b", "h", "01.2"], "invalid quantile \"01.2\"");
    assert_error(&["quantile", "b", "h", "+1"], "invalid quantile \"+1\"");
    assert_error(&["quantile", "b", "h", "-1"], "unknown option \"-1\"");
    assert_error(&["quantile", "b", "h", ".5"], "invalid quantile \".5\"");
    assert_error(&["quantile", "b", "h", "1."], "invalid quantile \"1.\"");
    assert_error(
        &["quantile", "b", "h", "1.0000"],
        "invalid quantile \"1.0000\"",
    );
    assert_error(
        &["quantile", "b", "h", "100.001"],
        "invalid quantile \"100.001\"",
    );
    assert_error(&["quantile", "b", "h", " 1"], "invalid quantile \" 1\"");
    assert_error(&["quantile", "b", "h", "1e2"], "invalid quantile \"1e2\"");
    assert_error(&["quantile", "b", "h", "١"], "invalid quantile \"١\"");
    assert_error(
        &["quantile", "b", "h", "18446744073709551616"],
        "invalid quantile \"18446744073709551616\"",
    );
    assert_error(
        &["quantile", "b", "h", "50", "invalid"],
        "invalid quantile \"invalid\"",
    );
}

#[test]
fn path_dash_validation_occurs_before_quantile_parsing_without_counting_query_dashes() {
    assert_error(&["quantile", "-", "-", "-"], "invalid quantile \"-\"");
}

#[test]
fn commands_parse_into_their_expected_public_variants() {
    assert_eq!(
        parse_words(&["merge", "--out=joined", "left", "right"]),
        Ok(Command::Merge {
            out: "joined".to_owned(),
            bundles: vec!["left".to_owned(), "right".to_owned()],
        })
    );
    assert_eq!(
        parse_words(&["show", "bundle"]),
        Ok(Command::Show {
            bundle: "bundle".to_owned(),
        })
    );
    assert_eq!(
        parse_words(&["top", "--limit=4", "bundle", "words"]),
        Ok(Command::Top {
            bundle: "bundle".to_owned(),
            name: "words".to_owned(),
            limit: Some(4),
        })
    );
    assert_eq!(
        parse_words(&["quantile", "bundle", "latency", "50"]),
        Ok(Command::Quantile {
            bundle: "bundle".to_owned(),
            name: "latency".to_owned(),
            quantiles: vec![("50".to_owned(), 50_000)],
        })
    );
    assert_eq!(
        parse_words(&["has", "bundle", "seen", "alpha"]),
        Ok(Command::Has {
            bundle: "bundle".to_owned(),
            name: "seen".to_owned(),
            values: vec!["alpha".to_owned()],
        })
    );
    assert_eq!(
        parse_words(&["verify", "one", "two"]),
        Ok(Command::Verify {
            bundles: vec!["one".to_owned(), "two".to_owned()],
        })
    );
}
