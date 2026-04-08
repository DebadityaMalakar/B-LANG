use std::fmt;

#[derive(Clone, Debug)]
pub struct SourceLocation {
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug)]
pub enum ErrorKind {
    Lex,
    Parse,
    Runtime,
}

#[derive(Clone, Debug)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
    pub location: Option<SourceLocation>,
}

impl Error {
    pub fn lex(message: impl Into<String>, location: Option<SourceLocation>) -> Self {
        Self {
            kind: ErrorKind::Lex,
            message: message.into(),
            location,
        }
    }

    pub fn parse(message: impl Into<String>, location: Option<SourceLocation>) -> Self {
        Self {
            kind: ErrorKind::Parse,
            message: message.into(),
            location,
        }
    }

    pub fn runtime(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Runtime,
            message: message.into(),
            location: None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.location {
            Some(loc) => write!(
                f,
                "{:?} error at line {}, column {}: {}",
                self.kind, loc.line, loc.column, self.message
            ),
            None => write!(f, "{:?} error: {}", self.kind, self.message),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug)]
pub enum RuntimeError {
    Message(String),
    Exit(i64),
}

impl RuntimeError {
    pub fn message(message: impl Into<String>) -> Self {
        RuntimeError::Message(message.into())
    }
}
