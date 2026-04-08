# B Language (b-lang)

A Rust-based MVP interpreter for the Bell Labs B programming language, paired with a VS Code extension for syntax highlighting and lightweight language tooling. This is a step-by-step build of the full language, so expect incremental progress before the implementation is fully usable.

## Project Status
- Work in progress, expanding the language in small, deliberate milestones.
- Early builds are intended for learning, experimentation, and feedback.

## Features
- Interpreter for a practical subset of B (expressions, control flow, functions, vectors).
- Built-in I/O and string helpers for the MVP runtime.
- VS Code extension with grammar-based highlighting and basic diagnostics.

## Implementation Checklist
- [x] Lexer and token diagnostics (unterminated strings/comments, delimiter mismatch)
- [x] Parser and AST for the MVP subset
- [x] Interpreter core (expressions, statements, control flow)
- [x] Function calls and stack frames
- [x] Vectors and subscripting
- [x] Built-in I/O and string helpers (MVP scope)
- [x] Example programs and tests
- [x] VS Code extension (syntax highlighting, hover, document symbols)
- [ ] Full B language feature parity
- [ ] Packed-character string model
- [ ] Extended file I/O beyond stdin/stdout

## Getting Started

### Prerequisites
- Rust (stable, 2024 edition)
- Node.js (18+ recommended) for the VS Code extension

### Run the Interpreter
```bash
cargo run -- path\to\program.b
```

### Run Tests
```bash
cargo test
```

### Build the VS Code Extension
```bash
cd extensions\b-lang-vscode
npm install
npm run compile
```

Open the extension folder in VS Code and launch the "Run Extension" debug target to test it.

## Licenses
- Rust interpreter code: GNU GPL v3 (see [LICENSE](LICENSE))
- VS Code extension: MIT (see [extensions/b-lang-vscode/LICENSE](extensions/b-lang-vscode/LICENSE))
