use std::fs;
use std::io::{BufReader, Cursor};

use b_lang::eval::{Interpreter, OutputSink};
use b_lang::parser::parse_program;

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

#[test]
fn arithmetic_loop() {
    let output = run_program("tests/programs/arith.b");
    assert_eq!(output, "01234");
}

#[test]
fn recursion_factorial() {
    let output = run_program("tests/programs/factorial.b");
    assert_eq!(output, "120");
}

#[test]
fn switch_case() {
    let output = run_program("tests/programs/switch.b");
    assert_eq!(output, "b");
}

#[test]
fn vector_string() {
    let output = run_program("tests/programs/vector_string.b");
    assert_eq!(output, "hi");
}

#[test]
fn goto_label() {
    let output = run_program("tests/programs/goto.b");
    assert_eq!(output, "3");
}
