use std::cell::RefCell;

use sketchfold::args::{Command, parse};
use sketchfold::cli::{Io, run};
use sketchfold::error::CliError;

const MANIFEST: &str = include_str!("../Cargo.toml");
const LOCK: &str = include_str!("../Cargo.lock");
const README: &str = include_str!("../README.md");

#[derive(Default)]
struct RecordingIo {
    stdout: RefCell<String>,
    stderr: RefCell<String>,
    touched: RefCell<Vec<String>>,
}

impl Io for RecordingIo {
    fn read_file(&self, path: &str) -> Option<Vec<u8>> {
        self.touched.borrow_mut().push(format!("read {path}"));
        None
    }

    fn read_stdin(&self) -> Vec<u8> {
        self.touched.borrow_mut().push("stdin".to_owned());
        Vec::new()
    }

    fn write_file(&self, path: &str, _data: &[u8]) -> bool {
        self.touched.borrow_mut().push(format!("write {path}"));
        false
    }

    fn out(&self, text: &str) {
        self.stdout.borrow_mut().push_str(text);
    }

    fn err(&self, text: &str) {
        self.stderr.borrow_mut().push_str(text);
    }
}

fn arguments(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn a_sole_version_argument_prints_the_package_version() {
    let io = RecordingIo::default();

    assert_eq!(run(&arguments(&["--version"]), &io), 0);
    assert_eq!(*io.stdout.borrow(), "sketchfold 0.2.0\n");
    assert_eq!(*io.stderr.borrow(), "");
    assert!(io.touched.borrow().is_empty());
}

#[test]
fn a_sole_version_argument_parses_to_the_version_command() {
    assert_eq!(parse(&arguments(&["--version"])), Ok(Command::Version));
}

#[test]
fn version_is_special_only_when_it_is_the_only_argument() {
    assert_eq!(
        parse(&arguments(&["--version", "show"])),
        Err(CliError("unknown command \"--version\"".to_owned()))
    );
    assert_eq!(
        parse(&arguments(&["--version", "--help"])),
        Err(CliError("unknown command \"--version\"".to_owned()))
    );
    assert_eq!(
        parse(&arguments(&["show", "--version"])),
        Err(CliError("unknown option \"--version\"".to_owned()))
    );
    assert_eq!(
        parse(&arguments(&["-V"])),
        Err(CliError("unknown command \"-V\"".to_owned()))
    );
    assert_eq!(
        parse(&arguments(&["--version=1"])),
        Err(CliError("unknown command \"--version=1\"".to_owned()))
    );
}

#[test]
fn a_refused_version_request_reports_on_standard_error_with_code_two() {
    let io = RecordingIo::default();

    assert_eq!(run(&arguments(&["--version", "show"]), &io), 2);
    assert_eq!(*io.stdout.borrow(), "");
    assert_eq!(
        *io.stderr.borrow(),
        "sketchfold: unknown command \"--version\"\n"
    );
}

#[test]
fn the_manifest_and_the_lock_file_say_version_0_2_0() {
    assert!(
        MANIFEST.contains(
            "[package]\nname = \"sketchfold\"\nversion = \"0.2.0\"\nedition = \"2024\"\n"
        )
    );
    assert!(LOCK.contains("[[package]]\nname = \"sketchfold\"\nversion = \"0.2.0\"\n"));
}

#[test]
fn the_readme_documents_the_version_flag() {
    assert!(README.contains(
        "`sketchfold --version` as the only argument prints `sketchfold 0.2.0` and exits with 0."
    ));
}
