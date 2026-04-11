use std::fs;
use std::io::{BufReader, Cursor};

use b_lang::error::RuntimeError;
use b_lang::eval::{Interpreter, OutputSink};
use b_lang::parser::parse_program;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn run_program(path: &str) -> String {
    let source = fs::read_to_string(path).expect("read program");
    let program = parse_program(&source).expect("parse program");
    let input = Box::new(BufReader::new(Cursor::new(Vec::new())));
    let output = OutputSink::Buffer(Vec::new());
    let mut interpreter = Interpreter::with_io(program, input, output);
    interpreter.run_main().expect("run program");
    let bytes = interpreter.take_output().unwrap_or_default();
    String::from_utf8(bytes).expect("utf8 output")
}

/// Run a program and return the `RuntimeError` it produced.  Panics if the
/// program succeeds instead of failing.
fn run_program_err(path: &str) -> RuntimeError {
    let source = fs::read_to_string(path).expect("read program");
    let program = parse_program(&source).expect("parse program");
    let input = Box::new(BufReader::new(Cursor::new(Vec::new())));
    let output = OutputSink::Buffer(Vec::new());
    let mut interpreter = Interpreter::with_io(program, input, output);
    interpreter
        .run_main()
        .expect_err("expected runtime error but program succeeded")
}

// ---------------------------------------------------------------------------
// Phase 1 & 2 regression tests
// ---------------------------------------------------------------------------

#[test]
fn arithmetic_loop() {
    assert_eq!(run_program("tests/programs/arith.b"), "01234");
}

#[test]
fn recursion_factorial() {
    assert_eq!(run_program("tests/programs/factorial.b"), "120");
}

#[test]
fn switch_case() {
    assert_eq!(run_program("tests/programs/switch.b"), "b");
}

#[test]
fn vector_string() {
    assert_eq!(run_program("tests/programs/vector_string.b"), "hi");
}

#[test]
fn goto_label() {
    assert_eq!(run_program("tests/programs/goto.b"), "3");
}

// ---------------------------------------------------------------------------
// Phase 3: error detection
// ---------------------------------------------------------------------------

#[test]
fn stack_overflow_detected() {
    // Run on a thread with a large native stack so the interpreter's
    // MAX_STACK_DEPTH limit is what triggers, not the OS stack limit.
    let err = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024) // 64 MB
        .spawn(|| run_program_err("tests/programs/stack_overflow.b"))
        .expect("spawn thread")
        .join()
        .expect("thread panicked");

    assert!(
        matches!(err, RuntimeError::StackOverflow),
        "expected StackOverflow, got: {:?}",
        err
    );
}

#[test]
fn division_by_zero_detected() {
    let err = run_program_err("tests/programs/divzero.b");
    assert!(
        matches!(err, RuntimeError::DivisionByZero),
        "expected DivisionByZero, got: {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// Phase 3: language coverage
// ---------------------------------------------------------------------------

#[test]
fn nested_control_with_fallthrough() {
    assert_eq!(run_program("tests/programs/nested_control.b"), "0122");
}

#[test]
fn string_operations() {
    assert_eq!(run_program("tests/programs/string_ops.b"), "hihello world");
}

// ---------------------------------------------------------------------------
// Phase 4: include system
// ---------------------------------------------------------------------------

#[test]
fn include_basic() {
    assert_eq!(run_program("tests/programs/include_basic.b"), "7");
}

#[test]
fn include_duplicate_is_noop() {
    assert_eq!(run_program("tests/programs/include_duplicate.b"), "3");
}

#[test]
fn include_unknown_library_is_error() {
    let err = run_program_err("tests/programs/include_unknown.b");
    // Any runtime error is acceptable; what matters is that main is never reached
    // (output is empty) and an error is returned.
    assert!(
        !matches!(err, RuntimeError::Exit(_)),
        "expected a real error, got exit: {:?}",
        err
    );
}

#[test]
fn include_in_function_is_parse_error() {
    let source = fs::read_to_string("tests/programs/include_in_function.b").expect("read");
    let result = parse_program(&source);
    assert!(result.is_err(), "expected parse error for include inside function");
}

// ---------------------------------------------------------------------------
// Phase 4: integer math
// ---------------------------------------------------------------------------

#[test]
fn math_integer_builtins() {
    assert_eq!(
        run_program("tests/programs/math_integers.b"),
        "42 6 12 1024 12 0"
    );
}

// ---------------------------------------------------------------------------
// Phase 4: fixed-point
// ---------------------------------------------------------------------------

#[test]
fn math_fixed_point() {
    assert_eq!(run_program("tests/programs/math_fixed.b"), "1");
}

// ---------------------------------------------------------------------------
// Phase 4: trig
// ---------------------------------------------------------------------------

#[test]
fn math_sin_half_pi() {
    assert_eq!(run_program("tests/programs/math_trig.b"), "65536");
}

// ---------------------------------------------------------------------------
// Phase 4: RNG determinism
// ---------------------------------------------------------------------------

#[test]
fn math_rng_deterministic() {
    // Two runs with the same seed must produce identical output.
    let out1 = run_program("tests/programs/math_rng.b");
    let out2 = run_program("tests/programs/math_rng.b");
    assert_eq!(out1, out2, "RNG output differs between runs with the same seed");
    // Values must be in [0, 100) — verify by parsing.
    for part in out1.split_whitespace() {
        let n: i64 = part.parse().expect("numeric RNG output");
        assert!((0..100).contains(&n), "randrange(0,100) out of range: {}", n);
    }
}

// ---------------------------------------------------------------------------
// Phase 4: edge cases (no panics)
// ---------------------------------------------------------------------------

#[test]
fn math_edge_cases() {
    assert_eq!(run_program("tests/programs/math_edge.b"), "0 0");
}
