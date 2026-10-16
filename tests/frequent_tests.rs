use sketchfold::frequent::Frequent;

#[test]
fn new_starts_empty_with_zero_slack() {
    let sketch = Frequent::new(2);

    assert_eq!(sketch.k, 2);
    assert_eq!(sketch.n, 0);
    assert!(sketch.items.is_empty());
    assert_eq!(sketch.slack(), 0);
}

#[test]
fn minimum_capacity_keeps_one_counter() {
    let mut sketch = Frequent::new(1);

    sketch.add(b"a");

    assert_eq!(sketch.items, vec![(b"a".to_vec(), 1)]);
    assert_eq!(sketch.n, 1);
    assert_eq!(sketch.slack(), 0);
}

#[test]
fn maximum_capacity_is_supported_without_premature_eviction() {
    let mut sketch = Frequent::new(1000);

    sketch.add(b"a");

    assert_eq!(sketch.k, 1000);
    assert_eq!(sketch.items, vec![(b"a".to_vec(), 1)]);
    assert_eq!(sketch.n, 1);
}

#[test]
fn duplicate_values_increment_the_existing_counter_at_capacity() {
    let mut sketch = Frequent::new(1);
    sketch.add(b"a");

    sketch.add(b"a");

    assert_eq!(sketch.items, vec![(b"a".to_vec(), 2)]);
    assert_eq!(sketch.n, 2);
    assert_eq!(sketch.slack(), 0);
}

#[test]
fn a_full_miss_decrements_every_counter_without_inserting_the_missed_value() {
    let mut sketch = Frequent::new(2);
    sketch.add(b"a");
    sketch.add(b"b");

    sketch.add(b"c");

    assert!(sketch.items.is_empty());
    assert_eq!(sketch.n, 3);
    assert_eq!(sketch.slack(), 1);
}

#[test]
fn k_two_sequence_a_b_a_c_keeps_only_one_a_counter() {
    let mut sketch = Frequent::new(2);
    sketch.add(b"a");
    sketch.add(b"b");
    sketch.add(b"a");

    sketch.add(b"c");

    assert_eq!(sketch.items, vec![(b"a".to_vec(), 1)]);
    assert_eq!(sketch.n, 4);
    assert_eq!(sketch.slack(), 1);
}

#[test]
fn equal_counts_are_ordered_by_raw_bytes_including_empty_and_invalid_utf8() {
    let mut sketch = Frequent::new(3);
    sketch.add(b"\xff");
    sketch.add(b"");
    sketch.add(b"\xfe");

    assert_eq!(
        sketch.items,
        vec![
            (b"".to_vec(), 1),
            (b"\xfe".to_vec(), 1),
            (b"\xff".to_vec(), 1)
        ]
    );
    assert_eq!(sketch.n, 3);
}

#[test]
fn adding_to_a_tied_counter_restores_count_descending_order() {
    let mut sketch = Frequent::new(3);
    sketch.add(b"b");
    sketch.add(b"a");
    sketch.add(b"c");

    sketch.add(b"c");

    assert_eq!(
        sketch.items,
        vec![(b"c".to_vec(), 2), (b"a".to_vec(), 1), (b"b".to_vec(), 1)]
    );
}

#[test]
fn slack_tracks_multiple_decrements_and_integer_division() {
    let mut sketch = Frequent::new(2);
    sketch.add(b"a");
    sketch.add(b"b");
    sketch.add(b"c");
    sketch.add(b"d");
    sketch.add(b"e");
    sketch.add(b"f");

    assert!(sketch.items.is_empty());
    assert_eq!(sketch.n, 6);
    assert_eq!(sketch.slack(), 2);
}

#[test]
fn slack_is_zero_when_stored_counts_account_for_all_values() {
    let sketch = Frequent {
        k: 2,
        n: 7,
        items: vec![(b"a".to_vec(), 4), (b"b".to_vec(), 3)],
    };

    assert_eq!(sketch.slack(), 0);
}

#[test]
fn slack_uses_a_nonnegative_deficit_even_when_public_state_counts_exceed_n() {
    let sketch = Frequent {
        k: 2,
        n: 4,
        items: vec![(b"a".to_vec(), u64::MAX), (b"b".to_vec(), 2)],
    };

    assert_eq!(sketch.slack(), 0);
}

#[test]
fn merging_an_empty_right_sketch_preserves_the_left_state() {
    let mut left = Frequent::new(2);
    left.add(b"a");
    left.add(b"b");
    let before = left.clone();
    let right = Frequent::new(2);
    let right_before = right.clone();

    left.merge(&right);

    assert_eq!(left, before);
    assert_eq!(right, right_before);
}

#[test]
fn merging_into_an_empty_left_sketch_copies_counters_without_changing_right() {
    let mut left = Frequent::new(2);
    let mut right = Frequent::new(2);
    right.add(b"b");
    right.add(b"a");
    right.add(b"b");
    let right_before = right.clone();

    left.merge(&right);

    assert_eq!(left, right_before);
    assert_eq!(right, right_before);
}

#[test]
fn merging_the_specified_overlapping_counters_reduces_at_index_k() {
    let mut left = Frequent {
        k: 2,
        n: 4,
        items: vec![(b"a".to_vec(), 3), (b"b".to_vec(), 1)],
    };
    let right = Frequent {
        k: 2,
        n: 3,
        items: vec![(b"c".to_vec(), 2), (b"a".to_vec(), 1)],
    };
    let right_before = right.clone();

    left.merge(&right);

    assert_eq!(left.items, vec![(b"a".to_vec(), 3), (b"c".to_vec(), 1)]);
    assert_eq!(left.n, 7);
    assert_eq!(left.slack(), 1);
    assert_eq!(right, right_before);
}

#[test]
fn merging_disjoint_counters_subtracts_the_k_plus_one_threshold() {
    let mut left = Frequent {
        k: 2,
        n: 4,
        items: vec![(b"a".to_vec(), 3), (b"b".to_vec(), 1)],
    };
    let right = Frequent {
        k: 2,
        n: 3,
        items: vec![(b"c".to_vec(), 2), (b"d".to_vec(), 1)],
    };

    left.merge(&right);

    assert_eq!(left.items, vec![(b"a".to_vec(), 2), (b"c".to_vec(), 1)]);
    assert_eq!(left.n, 7);
}

#[test]
fn merging_no_more_than_k_values_sums_counts_without_reduction() {
    let mut left = Frequent {
        k: 3,
        n: 4,
        items: vec![(b"a".to_vec(), 3), (b"b".to_vec(), 1)],
    };
    let right = Frequent {
        k: 3,
        n: 3,
        items: vec![(b"a".to_vec(), 1), (b"c".to_vec(), 2)],
    };

    left.merge(&right);

    assert_eq!(
        left.items,
        vec![(b"a".to_vec(), 4), (b"c".to_vec(), 2), (b"b".to_vec(), 1)]
    );
    assert_eq!(left.n, 7);
    assert_eq!(left.slack(), 0);
}

#[test]
fn a_tie_at_the_merge_threshold_removes_every_counter_at_that_count() {
    let mut left = Frequent {
        k: 2,
        n: 4,
        items: vec![(b"a".to_vec(), 2), (b"b".to_vec(), 2)],
    };
    let right = Frequent {
        k: 2,
        n: 2,
        items: vec![(b"c".to_vec(), 2)],
    };

    left.merge(&right);

    assert!(left.items.is_empty());
    assert_eq!(left.n, 6);
    assert_eq!(left.slack(), 2);
}

#[test]
fn merging_preserves_raw_byte_order_after_equal_counts_are_reduced() {
    let mut left = Frequent {
        k: 2,
        n: 3,
        items: vec![(b"\xff".to_vec(), 2), (b"".to_vec(), 1)],
    };
    let right = Frequent {
        k: 2,
        n: 2,
        items: vec![(b"\xfe".to_vec(), 2)],
    };

    left.merge(&right);

    assert_eq!(
        left.items,
        vec![(b"\xfe".to_vec(), 1), (b"\xff".to_vec(), 1)]
    );
    assert_eq!(left.n, 5);
}

#[test]
fn add_saturates_both_n_and_an_existing_counter_at_u64_max() {
    let mut sketch = Frequent {
        k: 1,
        n: u64::MAX,
        items: vec![(b"a".to_vec(), u64::MAX)],
    };

    sketch.add(b"a");

    assert_eq!(sketch.n, u64::MAX);
    assert_eq!(sketch.items, vec![(b"a".to_vec(), u64::MAX)]);
}

#[test]
fn merging_equal_counters_saturates_stored_count_and_total_n() {
    let mut left = Frequent {
        k: 1,
        n: u64::MAX,
        items: vec![(b"a".to_vec(), u64::MAX)],
    };
    let right = Frequent {
        k: 1,
        n: u64::MAX,
        items: vec![(b"a".to_vec(), u64::MAX)],
    };

    left.merge(&right);

    assert_eq!(left.n, u64::MAX);
    assert_eq!(left.items, vec![(b"a".to_vec(), u64::MAX)]);
}

#[test]
fn merge_reduces_exact_u128_totals_before_saturating_storage() {
    let mut left = Frequent {
        k: 2,
        n: u64::MAX,
        items: vec![(b"a".to_vec(), u64::MAX - 1), (b"b".to_vec(), 1)],
    };
    let right = Frequent {
        k: 2,
        n: 3,
        items: vec![(b"a".to_vec(), 2), (b"c".to_vec(), 1)],
    };

    left.merge(&right);

    assert_eq!(left.n, u64::MAX);
    assert_eq!(left.items, vec![(b"a".to_vec(), u64::MAX)]);
}

#[test]
fn merge_saturates_n_without_saturating_unrelated_counters_early() {
    let mut left = Frequent {
        k: 2,
        n: u64::MAX,
        items: vec![(b"a".to_vec(), 1)],
    };
    let right = Frequent {
        k: 2,
        n: 1,
        items: vec![(b"b".to_vec(), 1)],
    };

    left.merge(&right);

    assert_eq!(left.items, vec![(b"a".to_vec(), 1), (b"b".to_vec(), 1)]);
    assert_eq!(left.n, u64::MAX);
    assert_eq!(left.slack(), (u64::MAX - 2) / 3);
}
