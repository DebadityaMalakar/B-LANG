use std::env;
use std::fs;

use b_lang::eval::{Interpreter, OutputSink};
use b_lang::parser::parse_program;

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();
    let debug_memory = args.iter().any(|a| a == "--debug-memory");
    args.retain(|a| a != "--debug-memory");

    let path = match args.into_iter().next() {
        Some(path) => path,
        None => {
            eprintln!("Usage: b-lang [--debug-memory] <file.b>");
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
    interpreter.set_output(OutputSink::Stdout(std::io::BufWriter::new(
        std::io::stdout(),
    )));
    interpreter.set_debug_memory(debug_memory);

    if debug_memory {
        eprintln!("[DEBUG] Starting interpreter for: {}", path);
    }

    match interpreter.run_main() {
        Ok(_) => {
            if debug_memory {
                interpreter.dump_heap();
                interpreter.dump_stack();
            }
        }
        Err(err) => {
            if debug_memory {
                interpreter.dump_heap();
                interpreter.dump_stack();
            }
            eprintln!("Runtime error: {:?}", err);
            std::process::exit(1);
        }
    }
}
