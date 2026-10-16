#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatError {
    pub line: usize,
    pub message: String,
}
