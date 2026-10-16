use sketchfold::frequent::Frequent;
use sketchfold::hist::Hist;
use sketchfold::hll::Hll;
use sketchfold::member::Member;
use sketchfold::sketch::Sketch;

fn assert_rejected_merge(mut left: Sketch, right: Sketch, message: &str) {
    let left_before = left.clone();
    let right_before = right.clone();

    assert_eq!(left.merge(&right), Err(message.to_owned()));
    assert_eq!(left, left_before);
    assert_eq!(right, right_before);
}

fn assert_hist_rejects_without_mutation(sketch: &mut Sketch, value: &[u8]) {
    let before = sketch.clone();

    assert!(!sketch.add(value));
    assert_eq!(*sketch, before);
}

#[test]
fn metadata_reports_empty_states_and_parameter_boundaries() {
    let distinct_min = Sketch::Distinct(Hll::new(4));
    let distinct_max = Sketch::Distinct(Hll::new(16));
    let top_min = Sketch::Top(Frequent::new(1));
    let top_max = Sketch::Top(Frequent::new(1000));
    let hist_min = Sketch::Hist(Hist::new(1));
    let hist_max = Sketch::Hist(Hist::new(8));
    let member_min = Sketch::Member(Member::new(6));
    let member_max = Sketch::Member(Member::new(20));

    assert_eq!(distinct_min.kind(), "distinct");
    assert_eq!(distinct_min.param_text(), "p=4");
    assert_eq!(distinct_min.n(), 0);
    assert_eq!(distinct_max.kind(), "distinct");
    assert_eq!(distinct_max.param_text(), "p=16");
    assert_eq!(distinct_max.n(), 0);
    assert_eq!(top_min.kind(), "top");
    assert_eq!(top_min.param_text(), "k=1");
    assert_eq!(top_min.n(), 0);
    assert_eq!(top_max.kind(), "top");
    assert_eq!(top_max.param_text(), "k=1000");
    assert_eq!(top_max.n(), 0);
    assert_eq!(hist_min.kind(), "hist");
    assert_eq!(hist_min.param_text(), "s=1");
    assert_eq!(hist_min.n(), 0);
    assert_eq!(hist_max.kind(), "hist");
    assert_eq!(hist_max.param_text(), "s=8");
    assert_eq!(hist_max.n(), 0);
    assert_eq!(member_min.kind(), "member");
    assert_eq!(member_min.param_text(), "b=6");
    assert_eq!(member_min.n(), 0);
    assert_eq!(member_max.kind(), "member");
    assert_eq!(member_max.param_text(), "b=20");
    assert_eq!(member_max.n(), 0);
}

#[test]
fn metadata_tracks_wrapped_counts_after_additions() {
    let mut distinct = Sketch::Distinct(Hll::new(4));
    let mut top = Sketch::Top(Frequent::new(1));
    let mut hist = Sketch::Hist(Hist::new(1));
    let mut member = Sketch::Member(Member::new(6));

    assert!(distinct.add(b"a"));
    assert!(distinct.add(b"a"));
    assert!(top.add(b"a"));
    assert!(top.add(b"b"));
    assert!(hist.add(b"42"));
    assert!(member.add(b"a"));
    assert!(member.add(b""));

    assert_eq!(distinct.param_text(), "p=4");
    assert_eq!(distinct.n(), 2);
    assert_eq!(top.param_text(), "k=1");
    assert_eq!(top.n(), 2);
    assert_eq!(hist.param_text(), "s=1");
    assert_eq!(hist.n(), 1);
    assert_eq!(member.param_text(), "b=6");
    assert_eq!(member.n(), 2);
}

#[test]
fn distinct_dispatches_empty_ascii_duplicate_and_raw_bytes_unchanged() {
    let mut wrapped = Sketch::Distinct(Hll::new(4));
    let mut direct = Hll::new(4);

    assert!(wrapped.add(b""));
    direct.add(b"");
    assert!(wrapped.add(b"a"));
    direct.add(b"a");
    assert!(wrapped.add(b"a"));
    direct.add(b"a");
    assert!(wrapped.add(b"\xff"));
    direct.add(b"\xff");
    assert!(wrapped.add(b"\x00"));
    direct.add(b"\x00");

    assert_eq!(wrapped, Sketch::Distinct(direct));
    assert_eq!(wrapped.n(), 5);
}

#[test]
fn top_dispatches_empty_ascii_duplicate_and_raw_bytes_unchanged() {
    let mut wrapped = Sketch::Top(Frequent::new(3));
    let mut direct = Frequent::new(3);

    assert!(wrapped.add(b""));
    direct.add(b"");
    assert!(wrapped.add(b"a"));
    direct.add(b"a");
    assert!(wrapped.add(b"a"));
    direct.add(b"a");
    assert!(wrapped.add(b"\xff"));
    direct.add(b"\xff");
    assert!(wrapped.add(b"\x00"));
    direct.add(b"\x00");

    assert_eq!(wrapped, Sketch::Top(direct));
    assert_eq!(wrapped.n(), 5);
}

#[test]
fn member_dispatches_empty_ascii_duplicate_and_raw_bytes_unchanged() {
    let mut wrapped = Sketch::Member(Member::new(6));
    let mut direct = Member::new(6);

    assert!(wrapped.add(b""));
    direct.add(b"");
    assert!(wrapped.add(b"a"));
    direct.add(b"a");
    assert!(wrapped.add(b"a"));
    direct.add(b"a");
    assert!(wrapped.add(b"\xff"));
    direct.add(b"\xff");
    assert!(wrapped.add(b"\x00"));
    direct.add(b"\x00");

    assert_eq!(wrapped, Sketch::Member(direct));
    assert_eq!(wrapped.n(), 5);
}

#[test]
fn histogram_dispatches_canonical_u64_values_to_the_direct_algorithm() {
    let mut wrapped = Sketch::Hist(Hist::new(4));
    let mut direct = Hist::new(4);

    assert!(wrapped.add(b"0"));
    direct.add(0);
    assert!(wrapped.add(b"1"));
    direct.add(1);
    assert!(wrapped.add(b"42"));
    direct.add(42);
    assert!(wrapped.add(b"18446744073709551615"));
    direct.add(u64::MAX);

    assert_eq!(wrapped, Sketch::Hist(direct));
    assert_eq!(wrapped.n(), 4);
}

#[test]
fn histogram_rejects_invalid_forms_without_changing_empty_or_populated_state() {
    let mut empty = Sketch::Hist(Hist::new(4));
    assert_hist_rejects_without_mutation(&mut empty, b"");
    assert_hist_rejects_without_mutation(&mut empty, b"00");
    assert_hist_rejects_without_mutation(&mut empty, b"01");
    assert_hist_rejects_without_mutation(&mut empty, b"+1");
    assert_hist_rejects_without_mutation(&mut empty, b"-1");
    assert_hist_rejects_without_mutation(&mut empty, b" 1");
    assert_hist_rejects_without_mutation(&mut empty, b"1 ");
    assert_hist_rejects_without_mutation(&mut empty, b"1\t");
    assert_hist_rejects_without_mutation(&mut empty, b"1\n");
    assert_hist_rejects_without_mutation(&mut empty, b"1.0");
    assert_hist_rejects_without_mutation(&mut empty, "١".as_bytes());
    assert_hist_rejects_without_mutation(&mut empty, b"\xff");
    assert_hist_rejects_without_mutation(&mut empty, b"18446744073709551616");

    let mut populated = Sketch::Hist(Hist::new(4));
    assert!(populated.add(b"7"));
    assert!(populated.add(b"18446744073709551615"));
    assert_hist_rejects_without_mutation(&mut populated, b"");
    assert_hist_rejects_without_mutation(&mut populated, b"00");
    assert_hist_rejects_without_mutation(&mut populated, b"01");
    assert_hist_rejects_without_mutation(&mut populated, b"+1");
    assert_hist_rejects_without_mutation(&mut populated, b"-1");
    assert_hist_rejects_without_mutation(&mut populated, b" 1");
    assert_hist_rejects_without_mutation(&mut populated, b"1 ");
    assert_hist_rejects_without_mutation(&mut populated, b"1\t");
    assert_hist_rejects_without_mutation(&mut populated, b"1\n");
    assert_hist_rejects_without_mutation(&mut populated, b"1.0");
    assert_hist_rejects_without_mutation(&mut populated, "١".as_bytes());
    assert_hist_rejects_without_mutation(&mut populated, b"\xff");
    assert_hist_rejects_without_mutation(&mut populated, b"18446744073709551616");
}

#[test]
fn every_ordered_pair_of_different_kinds_has_the_exact_error_and_is_unchanged() {
    assert_rejected_merge(
        Sketch::Distinct(Hll::new(4)),
        Sketch::Top(Frequent::new(1)),
        "cannot merge distinct and top",
    );
    assert_rejected_merge(
        Sketch::Distinct(Hll::new(4)),
        Sketch::Hist(Hist::new(1)),
        "cannot merge distinct and hist",
    );
    assert_rejected_merge(
        Sketch::Distinct(Hll::new(4)),
        Sketch::Member(Member::new(6)),
        "cannot merge distinct and member",
    );
    assert_rejected_merge(
        Sketch::Top(Frequent::new(1)),
        Sketch::Distinct(Hll::new(4)),
        "cannot merge top and distinct",
    );
    assert_rejected_merge(
        Sketch::Top(Frequent::new(1)),
        Sketch::Hist(Hist::new(1)),
        "cannot merge top and hist",
    );
    assert_rejected_merge(
        Sketch::Top(Frequent::new(1)),
        Sketch::Member(Member::new(6)),
        "cannot merge top and member",
    );
    assert_rejected_merge(
        Sketch::Hist(Hist::new(1)),
        Sketch::Distinct(Hll::new(4)),
        "cannot merge hist and distinct",
    );
    assert_rejected_merge(
        Sketch::Hist(Hist::new(1)),
        Sketch::Top(Frequent::new(1)),
        "cannot merge hist and top",
    );
    assert_rejected_merge(
        Sketch::Hist(Hist::new(1)),
        Sketch::Member(Member::new(6)),
        "cannot merge hist and member",
    );
    assert_rejected_merge(
        Sketch::Member(Member::new(6)),
        Sketch::Distinct(Hll::new(4)),
        "cannot merge member and distinct",
    );
    assert_rejected_merge(
        Sketch::Member(Member::new(6)),
        Sketch::Top(Frequent::new(1)),
        "cannot merge member and top",
    );
    assert_rejected_merge(
        Sketch::Member(Member::new(6)),
        Sketch::Hist(Hist::new(1)),
        "cannot merge member and hist",
    );
}

#[test]
fn distinct_precision_mismatch_reverses_numbers_without_mutating_either_side() {
    assert_rejected_merge(
        Sketch::Distinct(Hll::new(12)),
        Sketch::Distinct(Hll::new(10)),
        "precision differs (12 vs 10)",
    );
    assert_rejected_merge(
        Sketch::Distinct(Hll::new(10)),
        Sketch::Distinct(Hll::new(12)),
        "precision differs (10 vs 12)",
    );
}

#[test]
fn top_capacity_mismatch_reverses_numbers_without_mutating_either_side() {
    assert_rejected_merge(
        Sketch::Top(Frequent::new(16)),
        Sketch::Top(Frequent::new(8)),
        "capacity differs (16 vs 8)",
    );
    assert_rejected_merge(
        Sketch::Top(Frequent::new(8)),
        Sketch::Top(Frequent::new(16)),
        "capacity differs (8 vs 16)",
    );
}

#[test]
fn histogram_parameter_mismatch_reverses_numbers_without_mutating_either_side() {
    assert_rejected_merge(
        Sketch::Hist(Hist::new(4)),
        Sketch::Hist(Hist::new(5)),
        "bits differs (4 vs 5)",
    );
    assert_rejected_merge(
        Sketch::Hist(Hist::new(5)),
        Sketch::Hist(Hist::new(4)),
        "bits differs (5 vs 4)",
    );
}

#[test]
fn member_size_mismatch_reverses_numbers_without_mutating_either_side() {
    assert_rejected_merge(
        Sketch::Member(Member::new(14)),
        Sketch::Member(Member::new(10)),
        "size differs (14 vs 10)",
    );
    assert_rejected_merge(
        Sketch::Member(Member::new(10)),
        Sketch::Member(Member::new(14)),
        "size differs (10 vs 14)",
    );
}

#[test]
fn distinct_compatible_merges_match_direct_algorithm_for_empty_populated_and_saturated_states() {
    let mut empty_left = Sketch::Distinct(Hll::new(4));
    let empty_right = Sketch::Distinct(Hll::new(4));
    let mut expected_empty_left = Hll::new(4);
    let expected_empty_right = Hll::new(4);
    let empty_right_before = empty_right.clone();

    assert_eq!(empty_left.merge(&empty_right), Ok(()));
    expected_empty_left.merge(&expected_empty_right);
    assert_eq!(empty_left, Sketch::Distinct(expected_empty_left.clone()));
    assert_eq!(empty_right, empty_right_before);

    assert!(empty_left.add(b"a"));
    expected_empty_left.add(b"a");
    assert_eq!(empty_left.merge(&empty_right), Ok(()));
    expected_empty_left.merge(&expected_empty_right);
    assert_eq!(empty_left, Sketch::Distinct(expected_empty_left));
    assert_eq!(empty_right, empty_right_before);

    let mut left = Sketch::Distinct(Hll::new(4));
    let mut expected_left = Hll::new(4);
    assert!(left.add(b"a"));
    expected_left.add(b"a");
    let mut right = Sketch::Distinct(Hll::new(4));
    let mut expected_right = Hll::new(4);
    assert!(right.add(b""));
    expected_right.add(b"");
    assert!(right.add(b"\xff"));
    expected_right.add(b"\xff");
    let right_before = right.clone();

    assert_eq!(left.merge(&right), Ok(()));
    expected_left.merge(&expected_right);
    assert_eq!(left, Sketch::Distinct(expected_left));
    assert_eq!(right, right_before);

    let mut saturated_left = Hll::new(4);
    saturated_left.n = u64::MAX;
    saturated_left.add(b"a");
    let mut saturated_right = Hll::new(4);
    saturated_right.add(b"b");
    let saturated_right_before = saturated_right.clone();
    let mut wrapped_saturated_left = Sketch::Distinct(saturated_left.clone());
    let wrapped_saturated_right = Sketch::Distinct(saturated_right.clone());

    assert_eq!(
        wrapped_saturated_left.merge(&wrapped_saturated_right),
        Ok(())
    );
    saturated_left.merge(&saturated_right);
    assert_eq!(wrapped_saturated_left, Sketch::Distinct(saturated_left));
    assert_eq!(wrapped_saturated_left.n(), u64::MAX);
    assert_eq!(saturated_right, saturated_right_before);
}

#[test]
fn top_compatible_merges_match_direct_algorithm_for_empty_populated_and_saturated_states() {
    let mut empty_left = Sketch::Top(Frequent::new(2));
    let empty_right = Sketch::Top(Frequent::new(2));
    let mut expected_empty_left = Frequent::new(2);
    let expected_empty_right = Frequent::new(2);
    let empty_right_before = empty_right.clone();

    assert_eq!(empty_left.merge(&empty_right), Ok(()));
    expected_empty_left.merge(&expected_empty_right);
    assert_eq!(empty_left, Sketch::Top(expected_empty_left.clone()));
    assert_eq!(empty_right, empty_right_before);

    assert!(empty_left.add(b"a"));
    expected_empty_left.add(b"a");
    assert_eq!(empty_left.merge(&empty_right), Ok(()));
    expected_empty_left.merge(&expected_empty_right);
    assert_eq!(empty_left, Sketch::Top(expected_empty_left));
    assert_eq!(empty_right, empty_right_before);

    let mut left = Sketch::Top(Frequent::new(2));
    let mut expected_left = Frequent::new(2);
    assert!(left.add(b"a"));
    expected_left.add(b"a");
    let mut right = Sketch::Top(Frequent::new(2));
    let mut expected_right = Frequent::new(2);
    assert!(right.add(b""));
    expected_right.add(b"");
    assert!(right.add(b"\xff"));
    expected_right.add(b"\xff");
    let right_before = right.clone();

    assert_eq!(left.merge(&right), Ok(()));
    expected_left.merge(&expected_right);
    assert_eq!(left, Sketch::Top(expected_left));
    assert_eq!(right, right_before);

    let mut saturated_left = Frequent {
        k: 1,
        n: u64::MAX,
        items: vec![(b"a".to_vec(), u64::MAX)],
    };
    let saturated_right = Frequent {
        k: 1,
        n: 1,
        items: vec![(b"a".to_vec(), 1)],
    };
    let saturated_right_before = saturated_right.clone();
    let mut wrapped_saturated_left = Sketch::Top(saturated_left.clone());
    let wrapped_saturated_right = Sketch::Top(saturated_right.clone());

    assert_eq!(
        wrapped_saturated_left.merge(&wrapped_saturated_right),
        Ok(())
    );
    saturated_left.merge(&saturated_right);
    assert_eq!(wrapped_saturated_left, Sketch::Top(saturated_left));
    assert_eq!(wrapped_saturated_left.n(), u64::MAX);
    assert_eq!(saturated_right, saturated_right_before);
}

#[test]
fn histogram_compatible_merges_match_direct_algorithm_for_empty_populated_and_saturated_states() {
    let mut empty_left = Sketch::Hist(Hist::new(4));
    let empty_right = Sketch::Hist(Hist::new(4));
    let mut expected_empty_left = Hist::new(4);
    let expected_empty_right = Hist::new(4);
    let empty_right_before = empty_right.clone();

    assert_eq!(empty_left.merge(&empty_right), Ok(()));
    expected_empty_left.merge(&expected_empty_right);
    assert_eq!(empty_left, Sketch::Hist(expected_empty_left.clone()));
    assert_eq!(empty_right, empty_right_before);

    assert!(empty_left.add(b"1"));
    expected_empty_left.add(1);
    assert_eq!(empty_left.merge(&empty_right), Ok(()));
    expected_empty_left.merge(&expected_empty_right);
    assert_eq!(empty_left, Sketch::Hist(expected_empty_left));
    assert_eq!(empty_right, empty_right_before);

    let mut left = Sketch::Hist(Hist::new(4));
    let mut expected_left = Hist::new(4);
    assert!(left.add(b"1"));
    expected_left.add(1);
    let mut right = Sketch::Hist(Hist::new(4));
    let mut expected_right = Hist::new(4);
    assert!(right.add(b"42"));
    expected_right.add(42);
    assert!(right.add(b"18446744073709551615"));
    expected_right.add(u64::MAX);
    let right_before = right.clone();

    assert_eq!(left.merge(&right), Ok(()));
    expected_left.merge(&expected_right);
    assert_eq!(left, Sketch::Hist(expected_left));
    assert_eq!(right, right_before);

    let mut saturated_left = Hist::new(4);
    saturated_left.add(1);
    saturated_left.n = u64::MAX;
    let mut saturated_right = Hist::new(4);
    saturated_right.add(u64::MAX);
    let saturated_right_before = saturated_right.clone();
    let mut wrapped_saturated_left = Sketch::Hist(saturated_left.clone());
    let wrapped_saturated_right = Sketch::Hist(saturated_right.clone());

    assert_eq!(
        wrapped_saturated_left.merge(&wrapped_saturated_right),
        Ok(())
    );
    saturated_left.merge(&saturated_right);
    assert_eq!(wrapped_saturated_left, Sketch::Hist(saturated_left));
    assert_eq!(wrapped_saturated_left.n(), u64::MAX);
    assert_eq!(saturated_right, saturated_right_before);
}

#[test]
fn member_compatible_merges_match_direct_algorithm_for_empty_populated_and_saturated_states() {
    let mut empty_left = Sketch::Member(Member::new(6));
    let empty_right = Sketch::Member(Member::new(6));
    let mut expected_empty_left = Member::new(6);
    let expected_empty_right = Member::new(6);
    let empty_right_before = empty_right.clone();

    assert_eq!(empty_left.merge(&empty_right), Ok(()));
    expected_empty_left.merge(&expected_empty_right);
    assert_eq!(empty_left, Sketch::Member(expected_empty_left.clone()));
    assert_eq!(empty_right, empty_right_before);

    assert!(empty_left.add(b"a"));
    expected_empty_left.add(b"a");
    assert_eq!(empty_left.merge(&empty_right), Ok(()));
    expected_empty_left.merge(&expected_empty_right);
    assert_eq!(empty_left, Sketch::Member(expected_empty_left));
    assert_eq!(empty_right, empty_right_before);

    let mut left = Sketch::Member(Member::new(6));
    let mut expected_left = Member::new(6);
    assert!(left.add(b"a"));
    expected_left.add(b"a");
    let mut right = Sketch::Member(Member::new(6));
    let mut expected_right = Member::new(6);
    assert!(right.add(b""));
    expected_right.add(b"");
    assert!(right.add(b"\xff"));
    expected_right.add(b"\xff");
    let right_before = right.clone();

    assert_eq!(left.merge(&right), Ok(()));
    expected_left.merge(&expected_right);
    assert_eq!(left, Sketch::Member(expected_left));
    assert_eq!(right, right_before);

    let mut saturated_left = Member::new(6);
    saturated_left.n = u64::MAX;
    saturated_left.add(b"a");
    let mut saturated_right = Member::new(6);
    saturated_right.add(b"\x00");
    let saturated_right_before = saturated_right.clone();
    let mut wrapped_saturated_left = Sketch::Member(saturated_left.clone());
    let wrapped_saturated_right = Sketch::Member(saturated_right.clone());

    assert_eq!(
        wrapped_saturated_left.merge(&wrapped_saturated_right),
        Ok(())
    );
    saturated_left.merge(&saturated_right);
    assert_eq!(wrapped_saturated_left, Sketch::Member(saturated_left));
    assert_eq!(wrapped_saturated_left.n(), u64::MAX);
    assert_eq!(saturated_right, saturated_right_before);
}
