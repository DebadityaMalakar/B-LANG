# B Language (b-lang)

A Rust-based interpreter for the Bell Labs B programming language, paired with a VS Code extension for syntax highlighting and lightweight language tooling. The project is built in deliberate phases — each phase is fully tested before the next begins.

## Credits and Disclosure
- Author: Debaditya
- Co-Author: Claude
- Co-Authored-By: Claude <noreply@anthropic.com>
- This project is open about AI-assisted contributions to help readers evaluate code provenance and risk.

## Project Status
- Work in progress — Phases 1–5 complete, Phase 6 next.
- Last updated: April 11, 2026

---

## Quick Start

### Prerequisites
- Rust (stable, 2024 edition)
- Node.js 18+ (VS Code extension only)

### Run the Interpreter
```bash
cargo run -- path/to/program.b
```

### Run Tests
```bash
cargo test
```

### Build the VS Code Extension
```bash
cd extensions/b-lang-vscode
npm install
npm run compile
```
Open the extension folder in VS Code and launch the **Run Extension** debug target.

### Debug Flags
| Flag | Effect |
|---|---|
| `--trace-exec` | Print every `CALL`, `RETURN`, `GOTO` to stderr |
| `--debug-memory` | Dump heap and stack frames on exit |
| `--strict-memory` | Log every memory read/write to stderr |
| `--strict-math` | Warn to stderr on math domain errors |

---

## Language Reference

### Escape sequences
B uses `*` as the escape character inside string and character literals.

| Escape | Value |
|---|---|
| `\*n` | newline |
| `\*t` | tab |
| `\*e` | EOT / string terminator (ASCII 4) |
| `\*0` | null byte |
| `\*"` | double-quote |
| `\*'` | single-quote |
| `\*\*` | literal `*` |

### include / use namespace

```b
include math          /* register the math library (bare names always active) */
include string        /* register the string library (prefixed names active)   */

use namespace string  /* activate bare names: strlen, strcmp, … */
```

Statement ordering is strict and enforced by the parser:
1. All `include` statements must precede `use namespace`
2. All `use namespace` statements must precede function and external definitions
3. `use namespace X` without a prior `include X` is a fatal error at startup
4. Duplicate `include` for the same library is silently ignored (include guard)

Calling a string function with namespace prefix (`string::strlen(s)`) always works after `include string`, regardless of `use namespace`.

### Built-in I/O (always available)
| Function | Description |
|---|---|
| `getchar()` | Read one byte from stdin; returns `-1` on EOF |
| `putchar(c)` | Write low 8 bits of `c` to stdout |
| `putnumbs(n)` | Print decimal integer |
| `printf(fmt, ...)` | Format string: `%d` `%o` `%c` `%s` `%%` |
| `getstr(s)` | Read a line into vector `s`, append `\*e` |
| `putstr(s)` | Print vector `s` until `\*e` |
| `char(s, n)` | Return n-th character of vector `s` |
| `lchar(s, n, c)` | Set n-th character of vector `s` to `c` |
| `concat(dst, s1, ...)` | Concatenate strings into `dst` |
| `getvec(n)` | Allocate heap vector of size `n+1`, return base |
| `rlsvec(v)` | Release heap vector (no-op in MVP) |
| `nargs()` | Return argument count of current function |
| `exit()` | Terminate interpreter |

### Math library (`include math`)
The math library uses a **Q16 fixed-point** convention: `real_value = bvalue / 65536`.

**Integer math** (raw `i64`, no scaling):

| Function | Description |
|---|---|
| `abs(n)` | Absolute value |
| `sign(n)` | `-1`, `0`, or `1` |
| `min(a,b)` / `max(a,b)` | Minimum / maximum |
| `clamp(n,lo,hi)` | Clamp to `[lo, hi]` |
| `divf(a,b)` | Floor division (toward −∞) |
| `modf(a,b)` | Floored modulo (same sign as divisor) |
| `pow(base,exp)` | Integer fast exponentiation |
| `sqrt(n)` | Integer square root (floor) |
| `cbrt(n)` | Integer cube root (signed floor) |
| `gcd(a,b)` / `lcm(a,b)` | GCD / LCM |
| `popcnt(n)` | Count set bits |
| `clz(n)` / `ctz(n)` | Leading / trailing zero count |
| `bswap(n)` | 64-bit byte swap |

**Fixed-point Q16:**

| Function | Description |
|---|---|
| `itofp(n)` | Integer → Q16 (`n << 16`) |
| `fptoi(n)` | Q16 → integer (truncate) |
| `fpmul(a,b)` | Q16 multiply |
| `fpdiv(a,b)` | Q16 divide |

**Trig** (angles in milliradians, results Q16):

| Function | Description |
|---|---|
| `sin(mr)` / `cos(mr)` / `tan(mr)` | Trig functions |
| `asin(n)` / `acos(n)` | Inverse trig (Q16 input → milliradians) |
| `atan2(y,x)` | atan2 (Q16 inputs → milliradians) |
| `sinh(n)` / `cosh(n)` / `tanh(n)` | Hyperbolic (Q16 in/out) |

**Logarithms / exponentials** (Q16 in and out):

| Function | Description |
|---|---|
| `ln(n)` | Natural log |
| `log2(n)` / `log10(n)` | Base-2 / base-10 log |
| `exp(n)` / `exp2(n)` | e^n / 2^n |

**RNG:**

| Function | Description |
|---|---|
| `srand(seed)` | Seed the xorshift64 generator |
| `rand()` | Next pseudorandom `i64` |
| `randrange(lo,hi)` | Random integer in `[lo, hi)` |

**Constants** (Q16):

| Function | Value | Real |
|---|---|---|
| `m_pi()` | 205887 | π |
| `m_e()` | 178145 | e |
| `m_phi()` | 106028 | φ |
| `m_ln2()` | 45426 | ln 2 |
| `m_sqrt2()` | 92682 | √2 |

**Fixed-point example:**
```b
include math

main() {
    auto pi, half;
    pi   = m_pi();          /* Q16 π = 205887       */
    half = itofp(1) / 2;    /* Q16 0.5 = 32768      */
    putnumbs(fptoi(fpmul(pi, half)));   /* prints 1 */
}
```

---

### String library (`include string`)

All functions operate on B-strings: heap vectors of `BValue` where each slot holds one ASCII character code, terminated by `BValue(4)` (`\*e`).

String functions that read a source accept any heap string (including interned literals).  
String functions that **write** to a destination require the destination to be heap-allocated via `getvec`.  
Functions that allocate a new result (`lpad`, `rpad`, `pad`, `repeat`) return a heap base index — the caller is responsible for releasing it with `rlsvec`.

**Usage patterns:**
```b
include string
use namespace string   /* optional: enables bare names */

main() {
    auto src, dst, result;
    src    = "hello world";      /* src = heap addr of literal   */
    dst    = getvec(30);         /* writable heap destination    */
    toupper(src, dst);
    putstr(dst);                 /* "HELLO WORLD"                */
    rlsvec(dst);

    result = repeat("ab", 3);   /* allocates new "ababab" string */
    putstr(result);             /* "ababab"                      */
    rlsvec(result);

    /* prefixed call, no use namespace needed: */
    putnumbs(string::strlen("hi"));   /* 2 */
}
```

**Inspection:**

| Function | Description |
|---|---|
| `strlen(s)` | Length of string `s` |
| `strcmp(a, b)` | Lexicographic compare; `< 0`, `0`, or `> 0` |
| `strcmpi(a, b)` | Case-insensitive compare |
| `startswith(s, pre)` | `1` if `s` starts with `pre` |
| `endswith(s, suf)` | `1` if `s` ends with `suf` |
| `contains(s, needle)` | `1` if `needle` appears in `s` |
| `indexof(s, needle)` | Index of first occurrence, or `-1` |
| `count(s, needle)` | Count of non-overlapping occurrences |

**Case conversion** (write to `dst`):

| Function | Description |
|---|---|
| `toupper(s, dst)` | Uppercase copy into `dst` |
| `tolower(s, dst)` | Lowercase copy into `dst` |
| `tocamel(s, dst)` | camelCase (space/`_` delimiters) |
| `tosnake(s, dst)` | snake_case (camelCase + space boundaries) |
| `totitle(s, dst)` | Title Case (each word capitalised) |
| `capitalize(s, dst)` | First character uppercased, rest unchanged |

**Search / replace** (write to `dst`):

| Function | Description |
|---|---|
| `replace(s, from, to, dst)` | Replace all occurrences |
| `replacen(s, from, to, n, dst)` | Replace at most `n` occurrences |

**Trim** (write to `dst`):

| Function | Description |
|---|---|
| `strip(s, dst)` | Remove leading and trailing whitespace |
| `lstrip(s, dst)` | Remove leading whitespace |
| `rstrip(s, dst)` | Remove trailing whitespace |

**Pad** (return new heap index):

| Function | Description |
|---|---|
| `lpad(s, width, ch)` | Left-pad to `width` using char `ch` |
| `rpad(s, width, ch)` | Right-pad to `width` using char `ch` |
| `pad(s, width, ch)` | Centre within `width` using char `ch` |

**Repetition / substring** (pad returns new heap; substr writes to `dst`):

| Function | Description |
|---|---|
| `repeat(s, n)` | New string: `s` repeated `n` times |
| `substr(s, start, len, dst)` | Copy `len` chars from `start` into `dst` |
| `slice(s, start, end, dst)` | Copy `[start, end)` into `dst` |

**Number conversion** (write to `dst` / parse from `s`):

| Function | Description |
|---|---|
| `itoa(n, dst)` | Integer → decimal string |
| `itoao(n, dst)` | Integer → octal string |
| `itoax(n, dst)` | Integer → lowercase hex string |
| `atoi(s)` | Parse decimal integer from string |

**Character classification** (operate on a single character code, not a string):

| Function | Description |
|---|---|
| `isalpha(c)` | `1` if alphabetic |
| `isdigit(c)` | `1` if decimal digit |
| `isalnum(c)` | `1` if alphanumeric |
| `isspace(c)` | `1` if ASCII whitespace |
| `isupper(c)` | `1` if uppercase letter |
| `islower(c)` | `1` if lowercase letter |

---

## Phase History

### Phase 1 — Core Interpreter
Lexer, parser, AST, and tree-walking evaluator for the MVP subset of B.
- Block comments, octal literals, B-style escapes (`\*n`, `\*e`, …)
- Statements: `if/else`, `while`, `switch/case/default` with fallthrough, `break`, `return`, label + `goto`
- Expressions: full operator table (arithmetic, bitwise, logical, ternary, assignments, `++/--`, `&`/`*`, subscripting)
- Storage: `auto` locals, `extrn` globals, heap-backed vectors, interned string literals

### Phase 2 — Memory Model
Typed address space: global segment, heap segment, local frame.  
Tagged `i64` addresses prevent mixing across segments.  
Auto-vector allocation on function entry; global vectors pre-allocated in heap.

### Phase 3 — Interpreter Stability
- Structured `RuntimeError` enum (`UndefinedVariable`, `InvalidMemoryAccess`, `DivisionByZero`, `StackOverflow`, `InvalidGoto`)
- `MAX_STACK_DEPTH = 128` — raises `StackOverflow` before the Rust native stack overflows
- `--trace-exec`: logs `CALL`, `RETURN`, `GOTO` and resolved label jumps to stderr
- `--strict-memory`: logs every memory read/write with address decoding
- Shift amounts masked to 63 bits; string reads have a 65 536-element safety cap

### Phase 4 — Math Library
- `include math` top-level statement; `include` keyword added to lexer
- Include guard (`HashSet`) and deferred error surfacing in `run_main`
- `src/math.rs`: 40+ math builtins across integer, fixed-point, trig, log/exp, hyperbolic, RNG, and constants
- `src/libraries.rs`: `resolve_include` registry — single extension point for future libraries
- `--strict-math` flag: warns on domain errors (`sqrt(-1)`, `ln(0)`, etc.) before returning the safe default
- Sine lookup table generated at startup via `OnceLock<[i64; 512]>`
- Math builtins dispatched via `HashMap<String, MathFn>` before the I/O match

### Phase 5 — Robust Include System and String Library
- `use namespace <name>` top-level statement activates bare names for a library
- Include guard: duplicate `include` silently skipped (first-registration wins)
- Circular include detection: infrastructure in place via `include_stack: Vec<String>`
- `use namespace` without prior `include` is a fatal `[error]` at startup
- `::` namespace separator in the lexer and parser: `string::strlen(s)` always works
- `src/string_lib.rs`: 34 string builtins — inspection, case conversion, trim, pad, repeat, substr, number conversion, character classification
- `Library` struct in `libraries.rs` carries four maps: `math_bare`, `math_namespaced`, `string_bare`, `string_namespaced`
- `StringBuiltinFn` type: separate from `MathFn`, receives `&mut heap` for direct heap operations
- Namespace conflict resolution: first `include` / first `use namespace` wins (stable, no warnings)
- 28 automated tests (Phases 1–5)

---

## Implementation Checklist

### Completed
- [x] Lexer: octal, char/string literals, B-style escapes, `include` keyword, `::` separator
- [x] Parser: full expression precedence, all statements, label + goto, namespace-qualified calls
- [x] Interpreter: stack frames, globals, heap vectors, string interning
- [x] Structured runtime errors with typed variants
- [x] Stack depth enforcement (`MAX_STACK_DEPTH = 128`)
- [x] Debug flags: `--trace-exec`, `--debug-memory`, `--strict-memory`, `--strict-math`
- [x] `include` system with guard, circular detection, and `use namespace` ordering
- [x] Math library: integer, fixed-point, trig, log/exp, RNG, constants
- [x] String library: inspection, case, trim, pad, repeat, substr, number conversion, classification
- [x] Namespace-aware `Library` struct with per-library `math_*` / `string_*` maps
- [x] 28 automated tests (Phases 1–5)
- [x] VS Code extension: syntax highlighting, hover, document symbols

### Planned
- [ ] Phase 6: Raylib bindings (`include raylib`)
- [ ] Packed-character string model (4 chars per word)
- [ ] Extended file I/O beyond stdin/stdout

---

## Source Layout

```
src/
  main.rs        CLI entry point and flag parsing
  lib.rs         Module declarations
  lexer.rs       Tokenizer (+ `::` ColonColon, `use`/`namespace` keywords)
  parser.rs      Recursive-descent parser + Pratt expressions
  ast.rs         AST node types
  symbol.rs      Symbol table types
  memory.rs      BValue, Frame, GlobalMemory, Heap, address encoding
  eval.rs        Tree-walking interpreter
  error.rs       Error / RuntimeError types
  builtins.rs    STRING_TERMINATOR constant
  math.rs        Math library builtins (Phase 4)
  string_lib.rs  String library builtins (Phase 5)
  libraries.rs   Namespace-aware Library registry (Phase 5)

tests/
  exec_tests.rs           Integration test harness (28 tests)
  programs/               B source programs used by tests
```

---

## Code Statistics

Statistics produced by [`cloc`](https://github.com/AlDanial/cloc):

| Category | Count |
|---|---|
| Rust source files | 13 |
| Lines of Rust (code) | 3 337 |
| Lines of Rust (comments) | 347 |
| Test programs | 28 |
| Automated tests | 28 |

---

## Licenses
- Rust interpreter: GNU GPL v3 ([LICENSE](LICENSE))
- VS Code extension: MIT ([extensions/b-lang-vscode/LICENSE](extensions/b-lang-vscode/LICENSE))
