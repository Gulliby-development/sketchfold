use crate::args::{self, Command};
use crate::build;
use crate::bundle::{Bundle, merge_all};
use crate::error::CliError;
use crate::format::{decode, encode};
use crate::report;
use crate::sketch::Sketch;
use crate::text::{escape_value, plural};

/// The input and output boundary used by the command runner.
pub trait Io {
    fn read_file(&self, path: &str) -> Option<Vec<u8>>;
    fn read_stdin(&self) -> Vec<u8>;
    fn write_file(&self, path: &str, data: &[u8]) -> bool;
    fn out(&self, text: &str);
    fn err(&self, text: &str);
}

/// Executes one validated command and returns its process exit code.
pub fn run(args: &[String], io: &dyn Io) -> i32 {
    let command = match args::parse(args) {
        Ok(command) => command,
        Err(error) => return fail(io, error),
    };

    match command {
        Command::Help => {
            io.out(concat!(
                "usage: sketchfold build --sketch NAME:KIND:FIELD[:PARAM]... [--delim C] --out BUNDLE [FILE...]\n",
                  "       sketchfold merge --out BUNDLE BUNDLE BUNDLE...\n",
                "       sketchfold show BUNDLE\n",
                "       sketchfold top [--limit N] BUNDLE NAME\n",
                "       sketchfold quantile BUNDLE NAME Q...\n",
                "       sketchfold has BUNDLE NAME VALUE...\n",
                "       sketchfold verify BUNDLE...\n",
            ));
            0
        }
        Command::Version => {
            io.out(concat!("sketchfold ", env!("CARGO_PKG_VERSION"), "\n"));
            0
        }
        Command::Build {
            specs,
            delim,
            out,
            inputs,
        } => run_build(&specs, &delim, &out, &inputs, io),
        Command::Merge { out, bundles } => run_merge(&out, &bundles, io),
        Command::Show { bundle } => match load_bundle(&bundle, io) {
            Ok(bundle) => {
                io.out(&report::show(&bundle));
                0
            }
            Err(error) => fail(io, CliError(error)),
        },
        Command::Top {
            bundle,
            name,
            limit,
        } => {
            let bundle = match load_bundle(&bundle, io) {
                Ok(bundle) => bundle,
                Err(error) => return fail(io, CliError(error)),
            };
            let Some(sketch) = bundle.get(&name) else {
                return fail(io, CliError(format!("{name}: no such sketch")));
            };
            let Sketch::Top(frequent) = sketch else {
                return fail(io, CliError(format!("{name}: not a top sketch")));
            };
            io.out(&report::top(&name, frequent, limit));
            0
        }
        Command::Quantile {
            bundle,
            name,
            quantiles,
        } => {
            let bundle = match load_bundle(&bundle, io) {
                Ok(bundle) => bundle,
                Err(error) => return fail(io, CliError(error)),
            };
            let Some(sketch) = bundle.get(&name) else {
                return fail(io, CliError(format!("{name}: no such sketch")));
            };
            let Sketch::Hist(histogram) = sketch else {
                return fail(io, CliError(format!("{name}: not a hist sketch")));
            };
            if histogram.n == 0 {
                return fail(io, CliError(format!("{name}: sketch is empty")));
            }

            let mut output = String::new();
            for (spelling, q) in quantiles {
                let Some(value) = histogram.quantile(q) else {
                    return fail(io, CliError(format!("{name}: sketch is empty")));
                };
                output.push_str(&format!("p{spelling} {value}\n"));
            }
            io.out(&output);
            0
        }
        Command::Has {
            bundle,
            name,
            values,
        } => {
            let bundle = match load_bundle(&bundle, io) {
                Ok(bundle) => bundle,
                Err(error) => return fail(io, CliError(error)),
            };
            let Some(sketch) = bundle.get(&name) else {
                return fail(io, CliError(format!("{name}: no such sketch")));
            };
            let Sketch::Member(member) = sketch else {
                return fail(io, CliError(format!("{name}: not a member sketch")));
            };

            let mut output = String::new();
            let mut found_missing = false;
            for value in values {
                let maybe = member.has(value.as_bytes());
                found_missing |= !maybe;
                output.push_str(&format!(
                    "{}: {}\n",
                    escape_value(value.as_bytes()),
                    if maybe { "maybe" } else { "no" }
                ));
            }
            io.out(&output);
            i32::from(found_missing)
        }
        Command::Verify { bundles } => run_verify(&bundles, io),
    }
}

fn run_build(
    specs: &[crate::spec::Spec],
    delim: &[u8],
    out: &str,
    paths: &[String],
    io: &dyn Io,
) -> i32 {
    let mut inputs = Vec::with_capacity(paths.len().max(1));
    if paths.is_empty() {
        inputs.push(io.read_stdin());
    } else {
        for path in paths {
            let Some(data) = read_path(path, io) else {
                return fail(
                    io,
                    CliError(format!("{}: cannot read the file", display_path(path))),
                );
            };
            inputs.push(data);
        }
    }

    let (bundle, stats) = build::build(specs, delim, &inputs);
    let encoded = encode(&bundle);
    if let Err(error) = write_bundle(out, &encoded, io) {
        return fail(io, error);
    }
    for (spec, stat) in specs.iter().zip(stats) {
        io.err(&format!(
            "{}: {}, {} skipped\n",
            spec.name,
            plural(stat.accepted, "value", "values"),
            stat.skipped
        ));
    }
    0
}

fn run_merge(out: &str, paths: &[String], io: &dyn Io) -> i32 {
    let mut bundles = Vec::with_capacity(paths.len());
    for path in paths {
        match load_bundle(path, io) {
            Ok(bundle) => bundles.push(bundle),
            Err(error) => return fail(io, CliError(error)),
        }
    }

    let merged = match merge_all(bundles) {
        Ok(bundle) => bundle,
        Err(error) => return fail(io, error),
    };
    let bundle_count = paths.len() as u64;
    let sketch_count = merged.entries.len() as u64;
    let encoded = encode(&merged);
    if let Err(error) = write_bundle(out, &encoded, io) {
        return fail(io, error);
    }
    io.err(&format!(
        "merged {}, {}\n",
        plural(bundle_count, "bundle", "bundles"),
        plural(sketch_count, "sketch", "sketches")
    ));
    0
}

fn run_verify(paths: &[String], io: &dyn Io) -> i32 {
    let mut failed = false;
    for path in paths {
        let display = display_path(path);
        match load_bundle(path, io) {
            Ok(bundle) => io.out(&format!(
                "{display}: ok ({})\n",
                plural(bundle.entries.len() as u64, "sketch", "sketches")
            )),
            Err(message) => {
                failed = true;
                io.out(&format!("{message}\n"));
            }
        }
    }
    i32::from(failed)
}

fn load_bundle(path: &str, io: &dyn Io) -> Result<Bundle, String> {
    let bytes = read_path(path, io)
        .ok_or_else(|| format!("{}: cannot read the file", display_path(path)))?;
    decode(&bytes)
        .map_err(|error| format!("{}:{}: {}", display_path(path), error.line, error.message))
}

fn read_path(path: &str, io: &dyn Io) -> Option<Vec<u8>> {
    if path == "-" {
        Some(io.read_stdin())
    } else {
        io.read_file(path)
    }
}

fn write_bundle(path: &str, text: &str, io: &dyn Io) -> Result<(), CliError> {
    if path == "-" {
        io.out(text);
        Ok(())
    } else if io.write_file(path, text.as_bytes()) {
        Ok(())
    } else {
        Err(CliError(format!(
            "{}: cannot write the file",
            display_path(path)
        )))
    }
}

fn display_path(path: &str) -> String {
    if path == "-" {
        "<stdin>".to_owned()
    } else {
        path.replace('\\', "/")
    }
}

fn fail(io: &dyn Io, error: CliError) -> i32 {
    io.err(&format!("sketchfold: {}\n", error.0));
    2
}
