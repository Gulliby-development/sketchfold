use sketchfold::bundle::{Bundle, Entry, merge_all};
use sketchfold::error::CliError;
use sketchfold::frequent::Frequent;
use sketchfold::hist::Hist;
use sketchfold::hll::Hll;
use sketchfold::member::Member;
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

fn assert_named_merge_error(name: &str, left: Sketch, right: Sketch, message: &str) {
    assert_eq!(
        merge_all(vec![
            bundle(vec![entry(name, left)]),
            bundle(vec![entry(name, right)]),
        ]),
        Err(CliError(format!("{name}: {message}"))),
    );
}

#[test]
fn get_returns_the_matching_sketch_and_none_for_absent_names() {
    let bundle = bundle(vec![
        entry("visits", Sketch::Distinct(Hll::new(4))),
        entry("routes", Sketch::Top(Frequent::new(2))),
    ]);

    assert_eq!(bundle.get("visits"), Some(&Sketch::Distinct(Hll::new(4))));
    assert_eq!(bundle.get("routes"), Some(&Sketch::Top(Frequent::new(2))));
    assert_eq!(bundle.get("missing"), None);
}

#[test]
fn merging_no_bundles_returns_an_empty_bundle() {
    assert_eq!(merge_all(vec![]), Ok(Bundle::default()));
}

#[test]
fn compatible_all_kind_merges_keep_first_seen_order_and_append_later_names() {
    let mut distinct_left = Sketch::Distinct(Hll::new(4));
    assert!(distinct_left.add(b"a"));
    let mut top_left = Frequent::new(2);
    top_left.add(b"x");
    top_left.add(b"x");
    let mut hist_left = Hist::new(4);
    hist_left.add(1);
    let mut member_left = Member::new(6);
    member_left.add(b"a");

    let mut distinct_right = Sketch::Distinct(Hll::new(4));
    assert!(distinct_right.add(b"b"));
    let mut top_right = Frequent::new(2);
    top_right.add(b"y");
    let mut hist_right = Hist::new(4);
    hist_right.add(20);
    let mut member_right = Member::new(6);
    member_right.add(b"b");
    let mut later_only = Frequent::new(1);
    later_only.add(b"only");

    let merged = merge_all(vec![
        bundle(vec![
            entry("distinct", distinct_left),
            entry("top", Sketch::Top(top_left)),
            entry("hist", Sketch::Hist(hist_left)),
            entry("member", Sketch::Member(member_left)),
        ]),
        bundle(vec![
            entry("member", Sketch::Member(member_right)),
            entry("later", Sketch::Top(later_only.clone())),
            entry("hist", Sketch::Hist(hist_right)),
            entry("top", Sketch::Top(top_right)),
            entry("distinct", distinct_right),
        ]),
    ])
    .unwrap();

    assert_eq!(merged.entries.len(), 5);
    assert_eq!(merged.entries[0].name, "distinct");
    assert_eq!(merged.entries[1].name, "top");
    assert_eq!(merged.entries[2].name, "hist");
    assert_eq!(merged.entries[3].name, "member");
    assert_eq!(merged.entries[4].name, "later");

    let mut expected_distinct = Hll::new(4);
    expected_distinct.add(b"a");
    expected_distinct.add(b"b");
    assert_eq!(
        merged.get("distinct"),
        Some(&Sketch::Distinct(expected_distinct))
    );

    let mut expected_top = Frequent::new(2);
    expected_top.add(b"x");
    expected_top.add(b"x");
    expected_top.add(b"y");
    assert_eq!(merged.get("top"), Some(&Sketch::Top(expected_top)));

    let mut expected_hist = Hist::new(4);
    expected_hist.add(1);
    expected_hist.add(20);
    assert_eq!(merged.get("hist"), Some(&Sketch::Hist(expected_hist)));

    let mut expected_member = Member::new(6);
    expected_member.add(b"a");
    expected_member.add(b"b");
    assert_eq!(merged.get("member"), Some(&Sketch::Member(expected_member)));
    assert_eq!(merged.get("later"), Some(&Sketch::Top(later_only)));
}

#[test]
fn top_merges_are_applied_left_to_right_instead_of_regrouped() {
    let first = bundle(vec![entry(
        "top",
        Sketch::Top(Frequent {
            k: 1,
            n: 10,
            items: vec![(b"a".to_vec(), 10)],
        }),
    )]);
    let second = bundle(vec![entry(
        "top",
        Sketch::Top(Frequent {
            k: 1,
            n: 9,
            items: vec![(b"b".to_vec(), 9)],
        }),
    )]);
    let third = bundle(vec![entry(
        "top",
        Sketch::Top(Frequent {
            k: 1,
            n: 8,
            items: vec![(b"c".to_vec(), 8)],
        }),
    )]);

    let left_to_right = merge_all(vec![first.clone(), second.clone(), third.clone()]).unwrap();
    let second_and_third = merge_all(vec![second, third]).unwrap();
    let regrouped = merge_all(vec![first, second_and_third]).unwrap();

    assert_eq!(
        left_to_right.get("top"),
        Some(&Sketch::Top(Frequent {
            k: 1,
            n: 27,
            items: vec![(b"c".to_vec(), 7)],
        })),
    );
    assert_eq!(
        regrouped.get("top"),
        Some(&Sketch::Top(Frequent {
            k: 1,
            n: 27,
            items: vec![(b"a".to_vec(), 9)],
        })),
    );
    assert_ne!(left_to_right.get("top"), regrouped.get("top"));
}

#[test]
fn kind_mismatch_errors_name_both_kinds_in_operand_order() {
    assert_named_merge_error(
        "combined",
        Sketch::Distinct(Hll::new(4)),
        Sketch::Top(Frequent::new(1)),
        "cannot merge distinct and top",
    );
    assert_named_merge_error(
        "combined",
        Sketch::Top(Frequent::new(1)),
        Sketch::Distinct(Hll::new(4)),
        "cannot merge top and distinct",
    );
}

#[test]
fn distinct_precision_mismatch_reports_both_orders() {
    assert_named_merge_error(
        "ips",
        Sketch::Distinct(Hll::new(12)),
        Sketch::Distinct(Hll::new(10)),
        "precision differs (12 vs 10)",
    );
    assert_named_merge_error(
        "ips",
        Sketch::Distinct(Hll::new(10)),
        Sketch::Distinct(Hll::new(12)),
        "precision differs (10 vs 12)",
    );
}

#[test]
fn top_capacity_mismatch_reports_both_orders() {
    assert_named_merge_error(
        "paths",
        Sketch::Top(Frequent::new(16)),
        Sketch::Top(Frequent::new(8)),
        "capacity differs (16 vs 8)",
    );
    assert_named_merge_error(
        "paths",
        Sketch::Top(Frequent::new(8)),
        Sketch::Top(Frequent::new(16)),
        "capacity differs (8 vs 16)",
    );
}

#[test]
fn histogram_parameter_mismatch_reports_both_orders() {
    assert_named_merge_error(
        "latency",
        Sketch::Hist(Hist::new(4)),
        Sketch::Hist(Hist::new(5)),
        "bits differs (4 vs 5)",
    );
    assert_named_merge_error(
        "latency",
        Sketch::Hist(Hist::new(5)),
        Sketch::Hist(Hist::new(4)),
        "bits differs (5 vs 4)",
    );
}

#[test]
fn member_size_mismatch_reports_both_orders() {
    assert_named_merge_error(
        "seen",
        Sketch::Member(Member::new(14)),
        Sketch::Member(Member::new(10)),
        "size differs (14 vs 10)",
    );
    assert_named_merge_error(
        "seen",
        Sketch::Member(Member::new(10)),
        Sketch::Member(Member::new(14)),
        "size differs (10 vs 14)",
    );
}

#[test]
fn merge_reports_the_first_mismatch_encountered_in_the_incoming_bundle() {
    let left = bundle(vec![
        entry("alpha", Sketch::Distinct(Hll::new(4))),
        entry("beta", Sketch::Top(Frequent::new(1))),
    ]);
    let right = bundle(vec![
        entry("beta", Sketch::Hist(Hist::new(1))),
        entry("alpha", Sketch::Top(Frequent::new(1))),
    ]);

    assert_eq!(
        merge_all(vec![left, right]),
        Err(CliError("beta: cannot merge top and hist".to_owned())),
    );
}
