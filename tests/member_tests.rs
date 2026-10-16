use sketchfold::member::Member;

fn set_positions(sketch: &Member) -> Vec<usize> {
    sketch
        .bits
        .iter()
        .enumerate()
        .filter_map(|(index, &set)| set.then_some(index))
        .collect()
}

#[test]
fn new_starts_empty_and_does_not_match_any_value() {
    let sketch = Member::new(6);

    assert_eq!(sketch.b, 6);
    assert_eq!(sketch.n, 0);
    assert_eq!(sketch.bits.len(), 64);
    assert!(set_positions(&sketch).is_empty());
    assert!(!sketch.has(b""));
    assert!(!sketch.has(b"a"));
}

#[test]
fn parameter_boundaries_allocate_the_expected_bit_counts() {
    let minimum = Member::new(6);
    let maximum = Member::new(20);

    assert_eq!(minimum.bits.len(), 64);
    assert_eq!(maximum.bits.len(), 1_048_576);
    assert!(set_positions(&minimum).is_empty());
    assert!(set_positions(&maximum).is_empty());
    assert_eq!(minimum.n, 0);
    assert_eq!(maximum.n, 0);
}

#[test]
fn a_uses_the_four_specified_positions_with_wrapping_double_hashing() {
    let mut sketch = Member::new(6);

    sketch.add(b"a");

    assert_eq!(set_positions(&sketch), vec![5, 18, 31, 56]);
    assert_eq!(sketch.n, 1);
    assert!(sketch.has(b"a"));
}

#[test]
fn duplicate_values_increment_n_without_changing_bits() {
    let mut sketch = Member::new(6);

    sketch.add(b"a");
    let bits_after_first_add = sketch.bits.clone();
    sketch.add(b"a");

    assert_eq!(sketch.n, 2);
    assert_eq!(sketch.bits, bits_after_first_add);
    assert!(sketch.has(b"a"));
}

#[test]
fn empty_and_malformed_utf8_values_are_kept_as_raw_bytes() {
    let mut sketch = Member::new(6);

    sketch.add(b"");
    sketch.add(b"\xff");

    assert_eq!(sketch.n, 2);
    assert!(sketch.has(b""));
    assert!(sketch.has(b"\xff"));
    assert!(!sketch.has(b"\xfe"));
}

#[test]
fn has_rejects_a_value_when_one_of_its_four_bits_is_missing() {
    let mut sketch = Member::new(6);
    sketch.bits[56] = true;
    sketch.bits[5] = true;
    sketch.bits[18] = true;

    assert!(!sketch.has(b"a"));
}

#[test]
fn has_accepts_all_four_positions_even_without_an_add() {
    let mut sketch = Member::new(6);
    sketch.bits[56] = true;
    sketch.bits[5] = true;
    sketch.bits[18] = true;
    sketch.bits[31] = true;

    assert_eq!(sketch.n, 0);
    assert!(sketch.has(b"a"));
}

#[test]
fn merging_two_empty_sketches_preserves_empty_state() {
    let mut left = Member::new(6);
    let right = Member::new(6);

    left.merge(&right);

    assert_eq!(left.n, 0);
    assert!(set_positions(&left).is_empty());
    assert_eq!(right.n, 0);
    assert!(set_positions(&right).is_empty());
}

#[test]
fn merging_an_empty_sketch_preserves_bits_and_the_right_operand() {
    let mut left = Member::new(6);
    left.add(b"a");
    let left_before_merge = left.clone();

    let right = Member::new(6);
    let right_before_merge = right.clone();

    left.merge(&right);

    assert_eq!(left, left_before_merge);
    assert_eq!(right, right_before_merge);
}

#[test]
fn merge_unions_overlapping_bits_and_adds_counts() {
    let mut left = Member::new(6);
    left.bits[3] = true;
    left.bits[5] = true;
    left.n = 2;

    let mut right = Member::new(6);
    right.bits[5] = true;
    right.bits[61] = true;
    right.n = 3;
    let right_before_merge = right.clone();

    left.merge(&right);

    assert_eq!(set_positions(&left), vec![3, 5, 61]);
    assert_eq!(left.n, 5);
    assert_eq!(right, right_before_merge);
}

#[test]
fn merge_matches_adding_the_same_handwritten_raw_values() {
    let mut left = Member::new(6);
    left.add(b"a");

    let mut right = Member::new(6);
    right.add(b"\xff");
    let right_before_merge = right.clone();

    left.merge(&right);

    let mut expected = Member::new(6);
    expected.add(b"a");
    expected.add(b"\xff");

    assert_eq!(left, expected);
    assert_eq!(left.n, 2);
    assert_eq!(right, right_before_merge);
}

#[test]
fn add_and_merge_counts_saturate_at_u64_max() {
    let mut left = Member::new(6);
    left.n = u64::MAX;
    left.add(b"a");

    let mut right = Member::new(6);
    right.n = 1;
    right.add(b"b");

    left.merge(&right);

    assert_eq!(left.n, u64::MAX);
    assert!(left.has(b"a"));
    assert!(left.has(b"b"));
    assert_eq!(right.n, 2);
    assert!(right.has(b"b"));
}
