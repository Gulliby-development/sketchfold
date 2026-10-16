<p align="center"><img src="docs/logo.svg" width="96" alt="" /></p>

# sketchfold

[![CI](https://github.com/Gulliby-development/sketchfold/actions/workflows/ci.yml/badge.svg)](https://github.com/Gulliby-development/sketchfold/actions/workflows/ci.yml) [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

sketchfold is an offline command-line tool for people who need compact summaries of line-oriented logs, exports, and other byte-oriented text. It builds deterministic, mergeable sketches so teams can summarize separate inputs and combine the results without keeping every input value. Input values remain bytes, including invalid UTF-8; output is deterministic and uses LF line endings on Windows, macOS, and Linux.

## Install

Build the release executable from the repository root:

```sh
cargo build --release
```

The executable is `target/release/sketchfold.exe` on Windows and `target/release/sketchfold` on macOS and Linux. Run it by that path, or put its containing directory on your PATH.

## Usage

With `--help` as the only argument, sketchfold prints this command synopsis:

```text
usage: sketchfold build --sketch NAME:KIND:FIELD[:PARAM]... [--delim C] --out BUNDLE [FILE...]
       sketchfold merge --out BUNDLE BUNDLE BUNDLE...
       sketchfold show BUNDLE
       sketchfold top [--limit N] BUNDLE NAME
       sketchfold quantile BUNDLE NAME Q...
       sketchfold has BUNDLE NAME VALUE...
       sketchfold verify BUNDLE...
```

For the examples below, save this input as `access.log`:

```text
10.0.0.1 /index.html 200 12
10.0.0.2 /index.html 200 15
10.0.0.1 /about 200 40
10.0.0.3 /index.html 304 3
10.0.0.2 /api/items 500 950
10.0.0.4 /index.html 200 18
10.0.0.1 /api/items 200 120
10.0.0.5 /about 404 7
10.0.0.3 /index.html 200 14
10.0.0.2 /api/items 200 130
```

Build the four sketches and inspect their report:

```sh
sketchfold build --sketch ips:distinct:1:4 --sketch paths:top:2:2 --sketch ms:hist:4:4 --sketch seen:member:2:6 --out access.sk access.log
sketchfold show access.sk
```

The report from `show access.sk` is:

```text
ips    distinct  p=4  n=10  about 6 distinct, error 26.0%
paths  top       k=2  n=10  2 counters, slack 2
ms     hist      s=4  n=10  min 3, max 950, p50 15, p90 135, p99 950
seen   member    b=6  n=10  12 of 64 bits set
```

This command builds only the `ips` sketch and writes its bundle to standard output:

```sh
sketchfold build --sketch ips:distinct:1:4 --out - access.log
```

The resulting bundle text is:

```text
sketchfold 1
sketch ips distinct p=4 n=10
regs 01000000000000010100000200030000
end c657f82f
```

## Sketches

A sketch specification has the form `NAME:KIND:FIELD[:PARAM]`. Names start with a lowercase ASCII letter and contain at most 32 characters; later characters may be lowercase letters, digits, underscores, or hyphens. Names must be unique in a bundle. KIND is `distinct`, `top`, `hist`, or `member`. FIELD is a zero-based field number from 0 through 99: field 0 is the whole line, while fields 1 through 99 select parts split by the delimiter. The default delimiter is one space. Set `--delim C` to use any one Unicode code point as the delimiter; its UTF-8 bytes are matched in each line.

| Kind | What it answers | Parameter and guarantee |
| --- | --- | --- |
| `distinct` | Approximately how many distinct byte values occurred. | HyperLogLog precision `p` is 4–16, default 12. Its approximate relative error is `1.04 / sqrt(2^p)`. |
| `top` | Which byte values were frequent, and their estimated counts. | Misra–Gries capacity `k` is 1–1000, default 16. Each retained counter is a lower bound; the true count is no greater than that counter plus the sketch's slack. |
| `hist` | The distribution and quantiles of integer values. | Integer-histogram parameter `s` is 1–8, default 4. Values below `2^(s+1)` are exact; larger buckets have relative width `2^-s`. Quantiles return a bucket's upper end, clamped to the exact observed minimum and maximum. |
| `member` | Whether a byte value may have occurred. | Bloom-filter size parameter `b` is 6–20, default 14. “Maybe” can be a false positive; “no” means the value was not added. |

A parameter can be omitted to use its kind's default. Sketches can be merged only when their kinds and corresponding parameters match: precision for `distinct`, capacity for `top`, `s` for `hist`, and `b` for `member`.

Input is read as raw bytes; invalid UTF-8 is not decoded. Each file is split at LF, one CR immediately before LF is removed, and a final line without LF is retained. Empty lines are ignored. A missing or empty selected field is skipped. For `hist`, a field is also skipped unless it is a canonical unsigned 64-bit decimal integer. The accepted integer range is 0 through 18446744073709551615: signs, leading zeroes such as `01`, non-digits, and overflow are rejected. There is no application-level input, file, line, or value size limit.

## Bundle file

Bundles are version 1 text: ASCII records separated by LF, in sketch order. The following is a syntax reference, not a complete bundle or an extra output example: choose only the records for each sketch kind. Uppercase words stand for values:

```text
sketchfold 1
sketch NAME distinct p=P n=N
regs HEX
sketch NAME top k=K n=N
item COUNT HEX
sketch NAME hist s=S n=N min=A max=B
bucket SHIFT TOP COUNT
sketch NAME member b=B n=N
bits HEX
end CHECKSUM
```

A distinct sketch has one `regs` record containing exactly `2 * 2^P` lowercase hexadecimal digits, two digits per register. A top sketch has zero to K `item` records; values are encoded as lowercase hex, ordered by descending count and then ascending value bytes. A histogram has `bucket` records ordered by ascending shift and then top value. A member sketch stores its `2^b` bits in exactly `2^b / 4` lowercase hexadecimal digits, four bits per digit; within each digit the lowest bit represents the lowest-numbered bit.

Numbers use canonical decimal notation. HEX uses lowercase digits. The final `end` record contains eight lowercase hexadecimal checksum digits: checksum32 (32-bit FNV-1a) over every byte before the `end` line, including the preceding LF. The end record itself must end in LF.

The decoder validates the header, record form and order, parameter ranges, register and bit lengths, duplicate names, count consistency, and checksum. Register values cannot exceed `65 - P`; item and bucket counts must be positive. Histogram bucket counts must sum to n; an empty histogram has minimum and maximum zero. For a histogram bucket, shift is at most `63 - s`; at shift zero, top is 0 through `2^(s+1)-1`, and at higher shifts it is `2^s` through `2^(s+1)-1`. It reports the first problem with a one-based physical line number, displayed as `BUNDLE:LINE: MESSAGE`. Messages include `not a sketchfold bundle`; `bad sketch line`; `duplicate sketch name "NAME"`; `bad regs line`, `bad bits line`, `bad item line`, or `bad bucket line`; `counts exceed n`; `bucket counts do not add up to n`; and `unexpected line`. Missing register or bit records are reported at the line where the sketch ends. The end-record errors are `bad end line`, `checksum mismatch`, `unexpected data after end`, and `missing end line`. Register values above `65 - P`, nonpositive item or bucket counts, and out-of-range or incorrectly ordered buckets are invalid records.

## Commands

- `build` accepts one or more repeatable `--sketch SPEC` options, optional `--delim C`, required `--out BUNDLE`, and zero or more input FILE paths. With no FILE paths it reads standard input. It writes the bundle to the selected output and reports each sketch on standard error in the form `NAME: ACCEPTED value(s), SKIPPED skipped`, using singular `value` only when ACCEPTED is one. When `--out -` is used, standard output contains only the bundle.
- `merge` takes a required `--out BUNDLE` and at least two bundle paths. It merges matching sketches from left to right and reports `merged BUNDLES bundle(s), SKETCHES sketch(es)` on standard error.
- `show BUNDLE` writes one aligned report line per sketch to standard output. The name is padded to the longest name, the kind to eight characters, and columns are separated by two spaces with no trailing space. Each line includes the parameter, observation count, and kind-specific detail: a rounded distinct estimate and one-decimal error percentage; top counter count and slack; histogram minimum, maximum, p50, p90, and p99 (or `empty` when n is zero); or member bits set.
- `top [--limit N] BUNDLE NAME` writes a header containing the name, n, and slack, followed by retained counters in sketch order. Each counter has a right-aligned count, two spaces, and an escaped value. The optional canonical `--limit` is 1–1000000.
- `quantile BUNDLE NAME Q...` accepts one or more quantiles from 0 to 100 with at most three decimal places. Each result has the form `pQ VALUE` and preserves the supplied spelling of Q. Signs, leading zeroes, a bare decimal point, and more than three decimal places are invalid.
- `has BUNDLE NAME VALUE...` tests one or more argument values as UTF-8 bytes and writes `ESCAPED: maybe` or `ESCAPED: no` for each. Its exit status is unsuccessful if any result is “no.”
- `verify BUNDLE...` checks each bundle, prints a result for every path on standard output, and continues after read or format failures. A result identifies an unreadable path, an invalid bundle as `BUNDLE:LINE: MESSAGE`, or a valid bundle as `BUNDLE: ok (COUNT sketch(es))`, using the singular form for one sketch.

Options are available only on the commands shown above: `--sketch SPEC` and `--delim C` for build, `--out BUNDLE` for build and merge, and `--limit N` for top. An option value may be the next argument even when it begins with a hyphen, or may follow an equals sign (for example, `--out=-`). Repeated `--sketch` options collect all specifications; the last value wins for other options. Unknown options—including short forms and options with an equals sign—are reported as typed. An argument beginning with a hyphen is not treated as an option value unless it is consumed as the preceding option's value. `--delim` must be exactly one code point. The default build delimiter is a space.

A FILE or BUNDLE path of `-` means standard input and may be used at most once among those input paths in a command. This restriction is independent of `--out -`, which means standard output, and of query values. Displayed paths replace backslashes with slashes; standard input is displayed as `<stdin>`. Build input files are processed in the supplied order, splitting each file independently.

Argument errors are reported before file access, in this order: no arguments gives `missing command (build, merge, show, top, quantile, has or verify)`; an invalid command gives `unknown command "ARG"`; then option scanning reports `unknown option "ARG"` or `<OPTION> needs a value` (for example, `--out needs a value`), from left to right. Next come `invalid --delim "VALUE"`, the first `invalid --sketch "SPEC"`, `duplicate sketch name "NAME"`, `missing --sketch`, and `invalid --limit "VALUE"` as applicable. Required-output and command-arity errors follow: `missing --out`, `merge needs at least two bundles`, `missing BUNDLE`, `missing NAME`, `missing Q`, `missing VALUE`, and `too many arguments`; then `- can be given once` and the first `invalid quantile "VALUE"`. Work errors come last: `PATH: cannot read the file`, `PATH: cannot write the file`, `BUNDLE:LINE: MESSAGE`, merge conflicts prefixed with the sketch name, `NAME: no such sketch`, `NAME: not a top sketch`, `NAME: not a hist sketch`, or `NAME: not a member sketch`, or `NAME: sketch is empty`. Merge conflicts say `cannot merge LEFT_KIND and RIGHT_KIND`, `precision differs (LEFT vs RIGHT)`, `capacity differs (LEFT vs RIGHT)`, `bits differs (LEFT vs RIGHT)`, or `size differs (LEFT vs RIGHT)`, as applicable.

Path, decoding, merge, and query errors are `CliError` failures: they are written to standard error as `sketchfold: MESSAGE`, leave standard output empty, and do not publish a bundle before validation succeeds. A real operating-system write failure is reported as a write error; no rollback of a partial real-file write is promised. Build and merge success summaries go to standard error. Bundle data, reports, queries, and verification results go to standard output.

## Exit codes

- `0`: the command succeeded.
- `1`: verification found an unreadable or invalid bundle, or at least one membership query returned “no.”
- `2`: a command-line, file, bundle, merge, or sketch-query error occurred. The diagnostic is written to standard error.

## Development

Run these checks from the repository root:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

## License

MIT, see [LICENSE](LICENSE).

---

Built with BAKED
