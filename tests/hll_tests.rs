use sketchfold::hll::Hll;

#[test]
fn new_starts_with_sixteen_zero_registers() {
    let sketch = Hll::new(4);

    assert_eq!(sketch.p, 4);
    assert_eq!(sketch.n, 0);
    assert_eq!(sketch.regs, vec![0; 16]);
}

#[test]
fn empty_estimate_is_zero_and_precision_four_error_is_260_tenths() {
    let sketch = Hll::new(4);

    assert_eq!(sketch.estimate(), 0);
    assert_eq!(sketch.error_tenths(), 260);
}

#[test]
fn precision_twelve_error_is_16_tenths() {
    assert_eq!(Hll::new(12).error_tenths(), 16);
}

#[test]
fn precision_boundaries_allocate_the_expected_register_counts() {
    let minimum = Hll::new(4);
    let maximum = Hll::new(16);

    assert_eq!(minimum.regs.len(), 16);
    assert_eq!(maximum.regs.len(), 65_536);
    assert_eq!(maximum.n, 0);
    assert_eq!(maximum.regs[0], 0);
    assert_eq!(maximum.regs[65_535], 0);
}

#[test]
fn empty_and_malformed_utf8_values_are_hashed_as_raw_bytes() {
    let mut sketch = Hll::new(4);

    sketch.add(b"");
    sketch.add(b"\xff");

    assert_eq!(sketch.n, 2);
    assert_eq!(
        sketch.regs,
        vec![0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]
    );
}

#[test]
fn adding_a_b_and_c_matches_the_specified_register_vector() {
    let mut sketch = Hll::new(4);

    sketch.add(b"a");
    sketch.add(b"b");
    sketch.add(b"c");

    assert_eq!(sketch.n, 3);
    assert_eq!(
        sketch.regs,
        vec![3, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0]
    );
}

#[test]
fn duplicate_values_increment_n_without_changing_registers() {
    let mut sketch = Hll::new(4);

    sketch.add(b"a");
    let registers_after_first_add = sketch.regs.clone();
    sketch.add(b"a");

    assert_eq!(sketch.n, 2);
    assert_eq!(sketch.regs, registers_after_first_add);
}

#[test]
fn estimate_uses_each_special_alpha_and_the_general_alpha() {
    let mut p4 = Hll::new(4);
    p4.regs = vec![1; 16];
    let mut p5 = Hll::new(5);
    p5.regs = vec![1; 32];
    let mut p6 = Hll::new(6);
    p6.regs = vec![1; 64];
    let mut p8 = Hll::new(8);
    p8.regs = vec![1; 256];

    assert_eq!(p4.estimate(), 22);
    assert_eq!(p5.estimate(), 45);
    assert_eq!(p6.estimate(), 91);
    assert_eq!(p8.estimate(), 368);
}

#[test]
fn raw_estimate_rounding_uses_floor_of_value_plus_one_half() {
    let mut sketch = Hll::new(4);
    sketch.regs = vec![2; 16];

    assert_eq!(sketch.estimate(), 43);
}

#[test]
fn small_range_correction_switches_at_two_point_five_times_register_count() {
    let mut raw_above_boundary = Hll::new(4);
    raw_above_boundary.regs = vec![0, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2];

    let mut raw_below_boundary = Hll::new(4);
    raw_below_boundary.regs = vec![0, 3, 3, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2];

    assert_eq!(raw_above_boundary.estimate(), 41);
    assert_eq!(raw_below_boundary.estimate(), 44);
}

#[test]
fn small_range_correction_uses_the_number_of_zero_registers() {
    let mut sketch = Hll::new(4);
    sketch.regs = vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    assert_eq!(sketch.estimate(), 1);
}

#[test]
fn merging_uses_registerwise_maxima_and_preserves_the_right_operand() {
    let mut left = Hll::new(4);
    left.add(b"a");
    left.add(b"b");

    let mut right = Hll::new(4);
    right.add(b"b");
    right.add(b"c");
    let right_before_merge = right.clone();

    left.merge(&right);

    assert_eq!(left.n, 4);
    assert_eq!(
        left.regs,
        vec![3, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(right, right_before_merge);
}

#[test]
fn merging_empty_sketches_preserves_both_empty_states() {
    let mut left = Hll::new(4);
    let right = Hll::new(4);

    left.merge(&right);

    assert_eq!(left.n, 0);
    assert_eq!(left.regs, vec![0; 16]);
    assert_eq!(right.n, 0);
    assert_eq!(right.regs, vec![0; 16]);
}

#[test]
fn merging_with_an_empty_sketch_preserves_registers() {
    let mut occupied = Hll::new(4);
    occupied.add(b"a");
    occupied.add(b"b");
    let expected = occupied.clone();
    let empty = Hll::new(4);

    occupied.merge(&empty);

    assert_eq!(occupied, expected);
    assert_eq!(empty.n, 0);
}

#[test]
fn adding_and_merging_counts_saturate_at_u64_max() {
    let mut left = Hll::new(4);
    left.n = u64::MAX;
    left.add(b"a");

    let mut right = Hll::new(4);
    right.n = 1;
    right.add(b"b");
    let right_before_merge = right.clone();

    left.merge(&right);

    assert_eq!(left.n, u64::MAX);
    assert_eq!(left.regs[0], 3);
    assert_eq!(right, right_before_merge);
}
