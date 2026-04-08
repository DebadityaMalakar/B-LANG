# B Language (b-lang)

A Rust-based MVP interpreter for the Bell Labs B programming language, paired with a VS Code extension for syntax highlighting and lightweight language tooling. This is a step-by-step build of the full language, so expect incremental progress before the implementation is fully usable.

## Credits and Disclosure
- Author: Debaditya
- Co-Author: Claude
- Co-Authored-By: Claude <noreply@anthropic.com>
- This project is open about AI-assisted contributions to help readers evaluate code provenance and risk.

## Project Status
- Work in progress, expanding the language in small, deliberate milestones.
- Early builds are intended for learning, experimentation, and feedback.
- Last updated: April 8, 2026

## Features
- Interpreter for a practical subset of B (expressions, control flow, functions, vectors).
- Built-in I/O and string helpers for the MVP runtime.
- CLI debug mode for heap/stack tracing (`--debug-memory`).
- VS Code extension with grammar-based highlighting and basic diagnostics.

## Language Coverage (MVP)
- Lexing: block comments, octal literals, and B-style escape sequences.
- Statements: `if/else`, `while`, `switch/case/default`, `break`, `return`, and label-based `goto`.
- Expressions: arithmetic, bitwise, logical, ternary, assignment ops, `++/--`, address/indirection, subscripting.
- Storage: `auto` locals, `extrn` globals, heap-backed vectors, interned string literals.
- Built-ins: `getchar`, `putchar`, `putnumbs`, `printf` (`%d/%o/%c/%s`), `char`, `lchar`, `getstr`, `putstr`, `concat`, `getvec`, `rlsvec`, `nargs`, `exit`.
- Stubs (return 0): `openr`, `openw`, `flush`, `reread`, `system`, `getarg`.

## Implementation Checklist
- [x] Lexer with octal numbers, char/string literals, and B-style escapes
- [x] Parser and AST for MVP expressions, statements, and declarations
- [x] Interpreter core (control flow, stack frames, globals, vectors)
- [x] Labels and `goto` for compound statements
- [x] Built-in I/O and string helpers (MVP scope)
- [x] Example programs and tests
- [x] VS Code extension (syntax highlighting, hover, document symbols)
- [ ] Packed-character string model
- [ ] Extended file I/O beyond stdin/stdout
- [ ] Broader B language compatibility and edge cases

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

## Code Statistics
```bash
	 10 text files.
	 10 unique files.
	  0 files ignored.

github.com/AlDanial/cloc v 2.06  T=0.05 s (216.9 files/s, 53216.0 lines/s)
-------------------------------------------------------------------------------
Language                     files          blank        comment
	code
-------------------------------------------------------------------------------
Rust                            10            195             47
	2212
-------------------------------------------------------------------------------
SUM:                            10            195             47
	2212
-------------------------------------------------------------------------------
```
