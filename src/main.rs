use std::io::{Read, Write};

use sketchfold::cli::{self, Io};

struct RealIo;

impl Io for RealIo {
    fn read_file(&self, path: &str) -> Option<Vec<u8>> {
        std::fs::read(path).ok()
    }

    fn read_stdin(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        let _ = std::io::stdin().read_to_end(&mut bytes);
        bytes
    }

    fn write_file(&self, path: &str, data: &[u8]) -> bool {
        std::fs::write(path, data).is_ok()
    }

    fn out(&self, text: &str) {
        let mut stdout = std::io::stdout().lock();
        let _ = stdout.write_all(text.as_bytes());
        let _ = stdout.flush();
    }

    fn err(&self, text: &str) {
        let mut stderr = std::io::stderr().lock();
        let _ = stderr.write_all(text.as_bytes());
        let _ = stderr.flush();
    }
}

fn main() {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    std::process::exit(cli::run(&args, &RealIo));
}
