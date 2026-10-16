use sketchfold::hist::Hist;

#[test]
fn new_starts_empty_with_zero_extrema() {
    let sketch = Hist::new(4);

    assert_eq!(sketch.s, 4);
    assert_eq!(sketch.n, 0);
    assert_eq!(sketch.min, 0);
    assert_eq!(sketch.max, 0);
    assert!(sketch.buckets.is_empty());
    assert_eq!(sketch.quantile(0), None);
    assert_eq!(sketch.quantile(100_000), None);
}

#[test]
fn minimum_precision_is_supported() {
    let mut sketch = Hist::new(1);

    sketch.add(1);

    assert_eq!(sketch.s, 1);
    assert_eq!(sketch.n, 1);
    assert_eq!(sketch.buckets, vec![(0, 1, 1)]);
}

#[test]
fn maximum_precision_is_supported() {
    let mut sketch = Hist::new(8);

    sketch.add(511);
    sketch.add(512);

    assert_eq!(sketch.s, 8);
    assert_eq!(sketch.buckets, vec![(0, 511, 1), (1, 256, 1)]);
}

#[test]
fn zero_is_an_exact_bucket_and_sets_both_extrema() {
    let mut sketch = Hist::new(1);

    sketch.add(0);

    assert_eq!(sketch.buckets, vec![(0, 0, 1)]);
    assert_eq!(sketch.n, 1);
    assert_eq!(sketch.min, 0);
    assert_eq!(sketch.max, 0);
}

#[test]
fn s_one_maps_zero_exact_values_and_coarse_values_to_sorted_buckets() {
    let mut sketch = Hist::new(1);
    sketch.add(5);
    sketch.add(0);
    sketch.add(4);
    sketch.add(3);

    assert_eq!(sketch.buckets, vec![(0, 0, 1), (0, 3, 1), (1, 2, 2)]);
    assert_eq!(sketch.n, 4);
    assert_eq!(sketch.min, 0);
    assert_eq!(sketch.max, 5);
}

#[test]
fn bucket_order_is_sorted_and_duplicate_keys_have_one_count() {
    let mut sketch = Hist::new(2);
    sketch.add(7);
    sketch.add(1);
    sketch.add(3);
    sketch.add(7);
    sketch.add(0);

    assert_eq!(
        sketch.buckets,
        vec![(0, 0, 1), (0, 1, 1), (0, 3, 1), (0, 7, 2)]
    );
    assert_eq!(sketch.n, 5);
}

#[test]
fn duplicate_values_increment_the_bucket_count_and_total() {
    let mut sketch = Hist::new(1);
    sketch.add(4);

    sketch.add(4);

    assert_eq!(sketch.buckets, vec![(1, 2, 2)]);
    assert_eq!(sketch.n, 2);
    assert_eq!(sketch.min, 4);
    assert_eq!(sketch.max, 4);
}

#[test]
fn exact_minimum_and_maximum_track_unsorted_updates() {
    let mut sketch = Hist::new(4);
    sketch.add(90);
    sketch.add(12);
    sketch.add(44);
    sketch.add(12);

    assert_eq!(sketch.min, 12);
    assert_eq!(sketch.max, 90);
    assert_eq!(sketch.n, 4);
}

#[test]
fn empty_quantiles_return_none_at_both_inclusive_endpoints() {
    let sketch = Hist::new(1);

    assert_eq!(sketch.quantile(0), None);
    assert_eq!(sketch.quantile(100_000), None);
}

#[test]
fn singleton_quantiles_return_the_exact_value_at_both_endpoints() {
    let mut sketch = Hist::new(1);
    sketch.add(u64::MAX);

    assert_eq!(sketch.quantile(0), Some(u64::MAX));
    assert_eq!(sketch.quantile(100_000), Some(u64::MAX));
}

#[test]
fn zero_quantile_selects_the_first_rank_and_clamps_to_the_minimum() {
    let mut sketch = Hist::new(1);
    sketch.add(4);
    sketch.add(6);

    assert_eq!(sketch.quantile(0), Some(5));
    assert_eq!(sketch.min, 4);
    assert_eq!(sketch.max, 6);
}

#[test]
fn endpoint_quantile_clamps_a_coarse_bucket_upper_bound_to_the_maximum() {
    let mut sketch = Hist::new(1);
    sketch.add(4);
    sketch.add(6);

    assert_eq!(sketch.quantile(100_000), Some(6));
}

#[test]
fn rank_boundary_changes_bucket_at_fifty_percent() {
    let mut sketch = Hist::new(1);
    sketch.add(4);
    sketch.add(6);
    let before = sketch.clone();

    assert_eq!(sketch.quantile(50_000), Some(5));
    assert_eq!(sketch.quantile(50_001), Some(6));
    assert_eq!(sketch, before);
}

#[test]
fn u64_max_uses_u128_bucket_endpoint_arithmetic() {
    let mut sketch = Hist::new(1);

    sketch.add(u64::MAX);

    assert_eq!(sketch.buckets, vec![(62, 3, 1)]);
    assert_eq!(sketch.min, u64::MAX);
    assert_eq!(sketch.max, u64::MAX);
    assert_eq!(sketch.quantile(0), Some(u64::MAX));
    assert_eq!(sketch.quantile(100_000), Some(u64::MAX));
}

#[test]
fn merging_an_empty_right_sketch_preserves_the_left_state() {
    let mut left = Hist::new(1);
    left.add(4);
    left.add(6);
    let before = left.clone();
    let right = Hist::new(1);
    let right_before = right.clone();

    left.merge(&right);

    assert_eq!(left, before);
    assert_eq!(right, right_before);
}

#[test]
fn merging_into_an_empty_left_sketch_copies_buckets_and_extrema() {
    let mut left = Hist::new(1);
    let mut right = Hist::new(1);
    right.add(0);
    right.add(6);
    let right_before = right.clone();

    left.merge(&right);

    assert_eq!(left, right_before);
    assert_eq!(right, right_before);
}

#[test]
fn merging_two_empty_sketches_keeps_zero_extrema_and_no_buckets() {
    let mut left = Hist::new(8);
    let right = Hist::new(8);

    left.merge(&right);

    assert_eq!(left.n, 0);
    assert_eq!(left.min, 0);
    assert_eq!(left.max, 0);
    assert!(left.buckets.is_empty());
}

#[test]
fn merging_overlapping_buckets_adds_counts_and_keeps_the_right_unchanged() {
    let mut left = Hist::new(1);
    left.add(4);
    left.add(6);
    let mut right = Hist::new(1);
    right.add(5);
    right.add(0);
    let right_before = right.clone();

    left.merge(&right);

    assert_eq!(left.buckets, vec![(0, 0, 1), (1, 2, 2), (1, 3, 1)]);
    assert_eq!(left.n, 4);
    assert_eq!(left.min, 0);
    assert_eq!(left.max, 6);
    assert_eq!(right, right_before);
}

#[test]
fn merging_disjoint_buckets_matches_adding_the_same_values_directly() {
    let mut left = Hist::new(1);
    left.add(1);
    left.add(4);
    let mut right = Hist::new(1);
    right.add(2);
    right.add(6);
    let right_before = right.clone();
    let mut direct = Hist::new(1);
    direct.add(1);
    direct.add(4);
    direct.add(2);
    direct.add(6);

    left.merge(&right);

    assert_eq!(left, direct);
    assert_eq!(right, right_before);
}

#[test]
fn merging_saturated_n_and_bucket_counts_does_not_overflow() {
    let mut left = Hist {
        s: 1,
        n: u64::MAX,
        min: 4,
        max: 4,
        buckets: vec![(1, 2, u64::MAX)],
    };
    let mut right = Hist::new(1);
    right.add(4);
    let right_before = right.clone();

    left.merge(&right);

    assert_eq!(left.n, u64::MAX);
    assert_eq!(left.buckets, vec![(1, 2, u64::MAX)]);
    assert_eq!(right, right_before);
}

#[test]
fn small_counts_merge_in_sorted_order_and_preserve_exact_extrema() {
    let mut left = Hist::new(2);
    left.add(12);
    left.add(1);
    let mut right = Hist::new(2);
    right.add(3);
    right.add(12);

    left.merge(&right);

    assert_eq!(left.buckets, vec![(0, 1, 1), (0, 3, 1), (1, 6, 2)]);
    assert_eq!(left.n, 4);
    assert_eq!(left.min, 1);
    assert_eq!(left.max, 12);
}

#[test]
fn large_count_rank_uses_u128_product_and_ceiling() {
    let first_count = 184_467_440_737_095_u64;
    let second_count = u64::MAX - first_count;
    let sketch = Hist {
        s: 1,
        n: u64::MAX,
        min: 0,
        max: 1,
        buckets: vec![(0, 0, first_count), (0, 1, second_count)],
    };

    assert_eq!(sketch.quantile(0), Some(0));
    assert_eq!(sketch.quantile(1), Some(1));
    assert_eq!(sketch.quantile(100_000), Some(1));
}

#[test]
fn cumulative_counts_use_u128_when_a_saturated_total_has_extra_bucket_counts() {
    let sketch = Hist {
        s: 1,
        n: u64::MAX,
        min: 0,
        max: 1,
        buckets: vec![(0, 0, u64::MAX - 1), (0, 1, 2)],
    };

    assert_eq!(sketch.quantile(100_000), Some(1));
}
