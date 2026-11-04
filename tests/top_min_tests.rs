use sketchfold::args::{Command, parse};
use sketchfold::error::CliError;
use sketchfold::frequent::Frequent;
use sketchfold::report::{top, top_with_min};

fn words(list: &[&str]) -> Result<Command, CliError> {
    let args: Vec<String> = list.iter().map(|word| (*word).to_owned()).collect();
    parse(&args)
}

fn sketch() -> Frequent {
    Frequent {
        k: 8,
        n: 40,
        items: vec![
            (b"alpha".to_vec(), 120),
            (b"beta".to_vec(), 30),
            (b"gamma".to_vec(), 30),
            (b"delta".to_vec(), 7),
            (b"line\n".to_vec(), 2),
        ],
    }
}

#[test]
fn min_keeps_counters_of_at_least_that_count() {
    assert_eq!(
        top_with_min("w", &sketch(), None, Some(30)),
        "w: n=40, slack 0\n120  alpha\n 30  beta\n 30  gamma\n"
    );
    assert_eq!(
        top_with_min("w", &sketch(), None, Some(31)),
        "w: n=40, slack 0\n120  alpha\n"
    );
    assert_eq!(
        top_with_min("w", &sketch(), None, Some(2)),
        "w: n=40, slack 0\n120  alpha\n 30  beta\n 30  gamma\n  7  delta\n  2  line\\x0a\n"
    );
}

#[test]
fn min_above_every_counter_shows_the_header_only() {
    assert_eq!(
        top_with_min("w", &sketch(), None, Some(121)),
        "w: n=40, slack 0\n"
    );
    assert_eq!(
        top_with_min("w", &sketch(), Some(2), Some(u64::MAX)),
        "w: n=40, slack 0\n"
    );
}

#[test]
fn min_is_applied_before_limit() {
    assert_eq!(
        top_with_min("w", &sketch(), Some(2), Some(30)),
        "w: n=40, slack 0\n120  alpha\n 30  beta\n"
    );
    assert_eq!(
        top_with_min("w", &sketch(), Some(1), Some(7)),
        "w: n=40, slack 0\n120  alpha\n"
    );
}

#[test]
fn without_min_the_report_is_unchanged() {
    assert_eq!(
        top_with_min("w", &sketch(), Some(1), None),
        top("w", &sketch(), Some(1)),
        "w: n=40, slack 0\n120  alpha\n"
    );
}

#[test]
fn min_is_parsed_with_limit_in_either_order() {
    let expected = Ok(Command::Top {
        bundle: "b".to_owned(),
        name: "w".to_owned(),
        limit: Some(3),
        min: Some(5),
    });
    assert_eq!(
        words(&["top", "--min", "5", "--limit", "3", "b", "w"]),
        expected
    );
    assert_eq!(words(&["top", "--limit=3", "b", "--min=5", "w"]), expected);
}

#[test]
fn last_min_wins_and_earlier_bad_values_are_discarded() {
    assert_eq!(
        words(&["top", "--min=0", "--min=9", "b", "w"]),
        Ok(Command::Top {
            bundle: "b".to_owned(),
            name: "w".to_owned(),
            limit: None,
            min: Some(9),
        })
    );
}

#[test]
fn min_accepts_the_largest_u64() {
    assert_eq!(
        words(&["top", "--min", "18446744073709551615", "b", "w"]),
        Ok(Command::Top {
            bundle: "b".to_owned(),
            name: "w".to_owned(),
            limit: None,
            min: Some(u64::MAX),
        })
    );
}

#[test]
fn min_values_must_be_canonical_counts_from_one() {
    for value in [
        "0",
        "01",
        "+1",
        "-1",
        "",
        "x",
        "1.5",
        "18446744073709551616",
    ] {
        assert_eq!(
            words(&["top", "--min", value, "b", "w"]),
            Err(CliError(format!("invalid --min {value:?}"))),
            "value {value:?}"
        );
    }
}

#[test]
fn bad_limit_is_reported_before_bad_min() {
    assert_eq!(
        words(&["top", "--min=0", "--limit=0", "b", "w"]),
        Err(CliError("invalid --limit \"0\"".to_owned()))
    );
    assert_eq!(
        words(&["top", "--limit=1", "--min=0", "b", "w"]),
        Err(CliError("invalid --min \"0\"".to_owned()))
    );
}

#[test]
fn min_needs_a_value_and_belongs_to_top_only() {
    assert_eq!(
        words(&["top", "b", "w", "--min"]),
        Err(CliError("--min needs a value".to_owned()))
    );
    assert_eq!(
        words(&["show", "--min", "1", "b"]),
        Err(CliError("unknown option \"--min\"".to_owned()))
    );
    assert_eq!(
        words(&["build", "--min=1", "--sketch=a:top:1", "--out=-"]),
        Err(CliError("unknown option \"--min=1\"".to_owned()))
    );
}
