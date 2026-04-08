use std::env;
use std::fs;

use b_lang::eval::{Interpreter, OutputSink};
use b_lang::parser::parse_program;

fn main() {
    let mut args = env::args().skip(1);
    let path = match args.next() {
        Some(path) => path,
        None => {
            eprintln!("Usage: b-lang <file.b>");
            std::process::exit(1);
        }
    };

    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("Failed to read {}: {}", path, err);
            std::process::exit(1);
        }
    };

    let program = match parse_program(&source) {
        Ok(program) => program,
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
    };

    let mut interpreter = Interpreter::new(program);
    interpreter.set_output(OutputSink::Stdout(std::io::BufWriter::new(std::io::stdout())));
    match interpreter.run_main() {
        Ok(_) => {}
        Err(err) => {
            eprintln!("Runtime error: {:?}", err);
            std::process::exit(1);
        }
    }
}
