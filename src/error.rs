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

/// Structured runtime error type.
///
/// Prefer the typed variants over `Message` when the error kind is known —
/// they produce better diagnostics and allow callers (tests) to pattern-match
/// on the specific failure.
#[derive(Debug, Clone)]
pub enum RuntimeError {
    /// A variable name was referenced that does not exist in any scope.
    UndefinedVariable(String),
    /// A memory load or store used an out-of-range or invalid address.
    InvalidMemoryAccess(i64),
    /// Integer division or remainder by zero.
    DivisionByZero,
    /// The interpreter call stack exceeded `MAX_STACK_DEPTH`.
    StackOverflow,
    /// A `goto` referenced a label that does not exist in the current function.
    InvalidGoto(String),
    /// General-purpose error message (catch-all for internal errors).
    Message(String),
    /// The B program called `exit()`.  Contains the exit code.
    Exit(i64),
}

impl RuntimeError {
    /// Construct a `Message` variant — kept for backwards-compatibility with
    /// sites where a structured variant has not yet been assigned.
    pub fn message(message: impl Into<String>) -> Self {
        RuntimeError::Message(message.into())
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::UndefinedVariable(name) => {
                write!(f, "undefined variable '{}'", name)
            }
            RuntimeError::InvalidMemoryAccess(addr) => {
                write!(f, "invalid memory access at address 0x{:x}", addr)
            }
            RuntimeError::DivisionByZero => write!(f, "division by zero"),
            RuntimeError::StackOverflow => write!(f, "stack overflow (max depth exceeded)"),
            RuntimeError::InvalidGoto(label) => {
                write!(f, "goto target '{}' not found in current function", label)
            }
            RuntimeError::Message(msg) => write!(f, "{}", msg),
            RuntimeError::Exit(code) => write!(f, "exit({})", code),
        }
    }
}

impl std::error::Error for RuntimeError {}
