const README: &str = include_str!("../README.md");

#[test]
fn usage_synopsis_matches_all_seven_lines_with_lf() {
    assert!(README.contains(concat!(
        "usage: sketchfold build --sketch NAME:KIND:FIELD[:PARAM]... [--delim C] --out BUNDLE [FILE...]\n",
        "       sketchfold merge --out BUNDLE BUNDLE BUNDLE...\n",
        "       sketchfold show BUNDLE\n",
        "       sketchfold top [--limit N] BUNDLE NAME\n",
        "       sketchfold quantile BUNDLE NAME Q...\n",
        "       sketchfold has BUNDLE NAME VALUE...\n",
        "       sketchfold verify BUNDLE...\n",
    )));
}

#[test]
fn access_log_example_matches_all_ten_lines_with_lf() {
    assert!(README.contains(concat!(
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
    )));
}

#[test]
fn show_report_example_preserves_alignment_and_all_four_lf_lines() {
    assert!(README.contains(concat!(
        "ips    distinct  p=4  n=10  about 6 distinct, error 26.0%\n",
        "paths  top       k=2  n=10  2 counters, slack 2\n",
        "ms     hist      s=4  n=10  min 3, max 950, p50 15, p90 135, p99 950\n",
        "seen   member    b=6  n=10  12 of 64 bits set\n",
    )));
}

#[test]
fn ips_bundle_example_preserves_all_four_lf_lines() {
    assert!(README.contains(concat!(
        "sketchfold 1\n",
        "sketch ips distinct p=4 n=10\n",
        "regs 01000000000000010100000200030000\n",
        "end c657f82f\n",
    )));
}

#[test]
fn all_seven_required_headings_are_present_exactly() {
    assert!(README.contains("\n## Install\n"));
    assert!(README.contains("\n## Usage\n"));
    assert!(README.contains("\n## Sketches\n"));
    assert!(README.contains("\n## Bundle file\n"));
    assert!(README.contains("\n## Commands\n"));
    assert!(README.contains("\n## Exit codes\n"));
    assert!(README.contains("\n## Development\n"));
}

#[test]
fn installation_platform_output_and_development_checks_are_documented() {
    assert!(README.contains("cargo build --release\n"));
    assert!(README.contains(&format!(
        "{}target/release/sketchfold.exe{} on Windows",
        char::from(96),
        char::from(96)
    )));
    assert!(README.contains(&format!(
        "{}target/release/sketchfold{} on macOS and Linux",
        char::from(96),
        char::from(96)
    )));
    assert!(README.contains("LF line endings on Windows, macOS, and Linux."));
    assert!(README.contains("cargo fmt --check\n"));
    assert!(README.contains("cargo clippy -- -D warnings\n"));
    assert!(README.contains("cargo test\n"));
}
