use std::collections::HashSet;

use crate::error::CliError;
use crate::report::parse_q;
use crate::spec::{Spec, parse_spec};
use crate::text::parse_count;

/// A fully validated command-line request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    Version,
    Build {
        specs: Vec<Spec>,
        delim: Vec<u8>,
        out: String,
        inputs: Vec<String>,
    },
    Merge {
        out: String,
        bundles: Vec<String>,
    },
    Show {
        bundle: String,
    },
    Top {
        bundle: String,
        name: String,
        limit: Option<usize>,
    },
    Quantile {
        bundle: String,
        name: String,
        quantiles: Vec<(String, u32)>,
    },
    Has {
        bundle: String,
        name: String,
        values: Vec<String>,
    },
    Verify {
        bundles: Vec<String>,
    },
}

#[derive(Default)]
struct Options {
    sketches: Vec<String>,
    delim: Option<String>,
    out: Option<String>,
    limit: Option<String>,
}

/// Parses arguments without performing any input or output.
pub fn parse(args: &[String]) -> Result<Command, CliError> {
    if args.is_empty() {
        return Err(error(
            "missing command (build, merge, show, top, quantile, has or verify)",
        ));
    }
    if args.len() == 1 && args[0] == "--help" {
        return Ok(Command::Help);
    }
    if args.len() == 1 && args[0] == "--version" {
        return Ok(Command::Version);
    }

    let command = args[0].as_str();
    if !matches!(
        command,
        "build" | "merge" | "show" | "top" | "quantile" | "has" | "verify"
    ) {
        return Err(error(format!("unknown command {command:?}")));
    }

    let (positionals, options) = scan_options(command, &args[1..])?;

    if command == "build"
        && options
            .delim
            .as_deref()
            .is_some_and(|delim| delim.chars().count() != 1)
    {
        return Err(error(format!(
            "invalid --delim {:?}",
            options.delim.as_deref().unwrap_or_default()
        )));
    }

    let mut specs = Vec::with_capacity(options.sketches.len());
    for spelling in &options.sketches {
        let Some(spec) = parse_spec(spelling) else {
            return Err(error(format!("invalid --sketch {spelling:?}")));
        };
        specs.push(spec);
    }

    let mut names = HashSet::with_capacity(specs.len());
    for spec in &specs {
        if !names.insert(spec.name.as_str()) {
            return Err(error(format!("duplicate sketch name {:?}", spec.name)));
        }
    }

    if command == "build" && specs.is_empty() {
        return Err(error("missing --sketch"));
    }

    let limit = if command == "top" {
        match options.limit.as_deref() {
            Some(value) => Some(
                parse_count(value, 1, 1_000_000)
                    .ok_or_else(|| error(format!("invalid --limit {value:?}")))?
                    as usize,
            ),
            None => None,
        }
    } else {
        None
    };

    match command {
        "build" => {
            let out = options.out.ok_or_else(|| error("missing --out"))?;
            reject_duplicate_stdin(&positionals)?;
            let delim = options.delim.unwrap_or_else(|| " ".to_owned()).into_bytes();
            Ok(Command::Build {
                specs,
                delim,
                out,
                inputs: positionals,
            })
        }
        "merge" => {
            let out = options.out.ok_or_else(|| error("missing --out"))?;
            if positionals.len() < 2 {
                return Err(error("merge needs at least two bundles"));
            }
            reject_duplicate_stdin(&positionals)?;
            Ok(Command::Merge {
                out,
                bundles: positionals,
            })
        }
        "show" => {
            let bundle = positionals
                .first()
                .ok_or_else(|| error("missing BUNDLE"))?
                .clone();
            if positionals.len() > 1 {
                return Err(error("too many arguments"));
            }
            reject_duplicate_stdin(&positionals)?;
            Ok(Command::Show { bundle })
        }
        "top" => {
            let bundle = positionals
                .first()
                .ok_or_else(|| error("missing BUNDLE"))?
                .clone();
            let name = positionals
                .get(1)
                .ok_or_else(|| error("missing NAME"))?
                .clone();
            if positionals.len() > 2 {
                return Err(error("too many arguments"));
            }
            reject_duplicate_stdin(&positionals[..1])?;
            Ok(Command::Top {
                bundle,
                name,
                limit,
            })
        }
        "quantile" => {
            let bundle = positionals
                .first()
                .ok_or_else(|| error("missing BUNDLE"))?
                .clone();
            let name = positionals
                .get(1)
                .ok_or_else(|| error("missing NAME"))?
                .clone();
            if positionals.len() < 3 {
                return Err(error("missing Q"));
            }
            reject_duplicate_stdin(&positionals[..1])?;
            let mut quantiles = Vec::with_capacity(positionals.len() - 2);
            for spelling in &positionals[2..] {
                let value = parse_q(spelling)
                    .ok_or_else(|| error(format!("invalid quantile {spelling:?}")))?;
                quantiles.push((spelling.clone(), value));
            }
            Ok(Command::Quantile {
                bundle,
                name,
                quantiles,
            })
        }
        "has" => {
            let bundle = positionals
                .first()
                .ok_or_else(|| error("missing BUNDLE"))?
                .clone();
            let name = positionals
                .get(1)
                .ok_or_else(|| error("missing NAME"))?
                .clone();
            if positionals.len() < 3 {
                return Err(error("missing VALUE"));
            }
            reject_duplicate_stdin(&positionals[..1])?;
            Ok(Command::Has {
                bundle,
                name,
                values: positionals[2..].to_vec(),
            })
        }
        "verify" => {
            if positionals.is_empty() {
                return Err(error("missing BUNDLE"));
            }
            reject_duplicate_stdin(&positionals)?;
            Ok(Command::Verify {
                bundles: positionals,
            })
        }
        _ => unreachable!("command was checked above"),
    }
}

fn scan_options(command: &str, args: &[String]) -> Result<(Vec<String>, Options), CliError> {
    let mut positionals = Vec::new();
    let mut options = Options::default();
    let mut index = 0;

    while index < args.len() {
        let argument = &args[index];
        if !argument.starts_with('-') || argument == "-" {
            positionals.push(argument.clone());
            index += 1;
            continue;
        }

        let (option, inline_value) = match argument.split_once('=') {
            Some((option, value)) => (option, Some(value.to_owned())),
            None => (argument.as_str(), None),
        };
        let offered = match (command, option) {
            ("build", "--sketch") => Some(0),
            ("build", "--delim") => Some(1),
            ("build" | "merge", "--out") => Some(2),
            ("top", "--limit") => Some(3),
            _ => None,
        };
        let Some(offered) = offered else {
            return Err(error(format!("unknown option {argument:?}")));
        };

        let value = match inline_value {
            Some(value) => value,
            None => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(error(format!("{option} needs a value")));
                };
                value.clone()
            }
        };
        match offered {
            0 => options.sketches.push(value),
            1 => options.delim = Some(value),
            2 => options.out = Some(value),
            3 => options.limit = Some(value),
            _ => unreachable!("all offered options have an assigned slot"),
        }
        index += 1;
    }

    Ok((positionals, options))
}

fn reject_duplicate_stdin(paths: &[String]) -> Result<(), CliError> {
    if paths.iter().filter(|path| path.as_str() == "-").count() > 1 {
        Err(error("- can be given once"))
    } else {
        Ok(())
    }
}

fn error(message: impl Into<String>) -> CliError {
    CliError(message.into())
}
