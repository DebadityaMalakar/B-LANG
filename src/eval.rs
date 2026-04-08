use crate::ast::{BinaryOp, Expr, Function, Program, Stmt, UnaryOp};
use crate::builtins::STRING_TERMINATOR;
use crate::error::RuntimeError;
use crate::memory::{
    add_offset, decode_address, encode_global, encode_local, Address, BValue, Frame, GlobalMemory,
};
use crate::symbol::{GlobalSymbol, LocalLayout, LocalSymbol};
use std::collections::HashMap;
use std::io::{self, BufRead, BufWriter, Read, Write};

pub enum OutputSink {
    Stdout(BufWriter<io::Stdout>),
    Buffer(Vec<u8>),
}

impl OutputSink {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
        match self {
            OutputSink::Stdout(writer) => writer
                .write_all(bytes)
                .map_err(|err| RuntimeError::message(err.to_string())),
            OutputSink::Buffer(buffer) => {
                buffer.extend_from_slice(bytes);
                Ok(())
            }
        }
    }

    fn flush(&mut self) -> Result<(), RuntimeError> {
        match self {
            OutputSink::Stdout(writer) => writer
                .flush()
                .map_err(|err| RuntimeError::message(err.to_string())),
            OutputSink::Buffer(_) => Ok(()),
        }
    }

    pub fn take_buffer(&mut self) -> Option<Vec<u8>> {
        match self {
            OutputSink::Buffer(buffer) => Some(std::mem::take(buffer)),
            _ => None,
        }
    }
}

pub struct Interpreter {
    globals: GlobalMemory,
    global_symbols: HashMap<String, GlobalSymbol>,
    functions: HashMap<String, Function>,
    local_layouts: HashMap<String, LocalLayout>,
    frames: Vec<Frame>,
    input: Box<dyn BufRead>,
    output: OutputSink,
    string_pool: HashMap<String, i64>,
}

impl Interpreter {
    pub fn new(program: Program) -> Self {
        let input = Box::new(io::BufReader::new(io::stdin()));
        let output = OutputSink::Stdout(BufWriter::new(io::stdout()));
        Self::with_io(program, input, output)
    }

    pub fn with_io(program: Program, input: Box<dyn BufRead>, output: OutputSink) -> Self {
        let (globals, global_symbols) = Self::init_globals(&program);
        let functions = program
            .functions
            .iter()
            .cloned()
            .map(|func| (func.name.clone(), func))
            .collect();
        let local_layouts = program
            .functions
            .iter()
            .map(|func| (func.name.clone(), Self::build_local_layout(func)))
            .collect();

        Self {
            globals,
            global_symbols,
            functions,
            local_layouts,
            frames: Vec::new(),
            input,
            output,
            string_pool: HashMap::new(),
        }
    }

    pub fn set_output(&mut self, output: OutputSink) {
        self.output = output;
    }

    pub fn run_main(&mut self) -> Result<BValue, RuntimeError> {
        match self.call_function("main", Vec::new()) {
            Ok(value) => Ok(value),
            Err(RuntimeError::Exit(code)) => Ok(BValue(code)),
            Err(err) => Err(err),
        }
    }

    pub fn take_output(&mut self) -> Option<Vec<u8>> {
        self.output.take_buffer()
    }

    fn init_globals(program: &Program) -> (GlobalMemory, HashMap<String, GlobalSymbol>) {
        let mut globals = GlobalMemory::new();
        let mut symbols = HashMap::new();
        for decl in &program.globals {
            let slot = globals.allocate_block(1);
            let mut symbol = GlobalSymbol {
                slot,
                is_vector: decl.vector_size.is_some(),
                vector_base: None,
            };
            if let Some(size) = decl.vector_size {
                let base = globals.allocate_block(size + 1);
                globals.data[slot] = BValue(encode_global(base));
                symbol.vector_base = Some(base);
            }
            symbols.insert(decl.name.clone(), symbol);
        }
        (globals, symbols)
    }

    fn build_local_layout(function: &Function) -> LocalLayout {
        let mut symbols = HashMap::new();
        let mut total_slots = 0;

        for param in &function.params {
            symbols.insert(
                param.clone(),
                LocalSymbol {
                    slot: total_slots,
                    is_vector: false,
                    vector_base: None,
                },
            );
            total_slots += 1;
        }

        for local in &function.locals {
            let slot = total_slots;
            total_slots += 1;
            let (is_vector, vector_base) = match local.vector_size {
                Some(size) => {
                    let base = total_slots;
                    total_slots += size + 1;
                    (true, Some(base))
                }
                None => (false, None),
            };
            symbols.insert(
                local.name.clone(),
                LocalSymbol {
                    slot,
                    is_vector,
                    vector_base,
                },
            );
        }

        LocalLayout {
            symbols,
            total_slots,
        }
    }

    fn call_function(&mut self, name: &str, args: Vec<BValue>) -> Result<BValue, RuntimeError> {
        if let Some(value) = self.call_builtin(name, &args)? {
            return Ok(value);
        }

        let function = self
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| RuntimeError::message(format!("undefined function {}", name)))?;
        let layout = self
            .local_layouts
            .get(name)
            .ok_or_else(|| RuntimeError::message("missing layout"))?
            .clone();

        let mut frame = Frame {
            func: name.to_string(),
            locals: vec![BValue(0); layout.total_slots],
            nargs: args.len(),
        };

        for (idx, param) in function.params.iter().enumerate() {
            if let Some(symbol) = layout.symbols.get(param) {
                frame.locals[symbol.slot] = args.get(idx).copied().unwrap_or(BValue(0));
            }
        }

        for symbol in layout.symbols.values() {
            if symbol.is_vector {
                if let Some(base) = symbol.vector_base {
                    frame.locals[symbol.slot] = BValue(encode_local(base));
                }
            }
        }

        self.frames.push(frame);
        let flow = self.exec_stmt(&function.body);
        self.frames.pop();

        match flow? {
            ControlFlow::Return(value) => Ok(value),
            ControlFlow::Next => Ok(BValue(0)),
            ControlFlow::Break => Ok(BValue(0)),
            ControlFlow::Goto(label) => Err(RuntimeError::message(format!(
                "unresolved goto label {}",
                label
            ))),
        }
    }

    fn exec_stmt(&mut self, stmt: &Stmt) -> Result<ControlFlow, RuntimeError> {
        match stmt {
            Stmt::Compound(stmts) => self.exec_compound(stmts),
            Stmt::If(cond, then_stmt, else_stmt) => {
                let value = self.eval_expr(cond)?.as_i64();
                if value != 0 {
                    self.exec_stmt(then_stmt)
                } else if let Some(else_stmt) = else_stmt {
                    self.exec_stmt(else_stmt)
                } else {
                    Ok(ControlFlow::Next)
                }
            }
            Stmt::While(cond, body) => {
                loop {
                    let value = self.eval_expr(cond)?.as_i64();
                    if value == 0 {
                        break;
                    }
                    match self.exec_stmt(body)? {
                        ControlFlow::Next => {}
                        ControlFlow::Break => break,
                        ControlFlow::Return(value) => return Ok(ControlFlow::Return(value)),
                        ControlFlow::Goto(label) => return Ok(ControlFlow::Goto(label)),
                    }
                }
                Ok(ControlFlow::Next)
            }
            Stmt::Switch(expr, body) => self.exec_switch(expr, body),
            Stmt::Break => Ok(ControlFlow::Break),
            Stmt::Return(expr) => {
                let value = match expr {
                    Some(expr) => self.eval_expr(expr)?,
                    None => BValue(0),
                };
                Ok(ControlFlow::Return(value))
            }
            Stmt::Goto(label) => Ok(ControlFlow::Goto(label.clone())),
            Stmt::Expr(expr) => {
                self.eval_expr(expr)?;
                Ok(ControlFlow::Next)
            }
            Stmt::Label(_, stmt) => self.exec_stmt(stmt),
            Stmt::Case(_, stmt) => self.exec_stmt(stmt),
            Stmt::Default(stmt) => self.exec_stmt(stmt),
        }
    }

    fn exec_compound(&mut self, stmts: &[Stmt]) -> Result<ControlFlow, RuntimeError> {
        let mut label_map = HashMap::new();
        for (idx, stmt) in stmts.iter().enumerate() {
            if let Stmt::Label(name, _) = stmt {
                label_map.insert(name.clone(), idx);
            }
        }

        let mut index = 0;
        while index < stmts.len() {
            let stmt = &stmts[index];
            match self.exec_stmt(stmt)? {
                ControlFlow::Next => index += 1,
                ControlFlow::Break => return Ok(ControlFlow::Break),
                ControlFlow::Return(value) => return Ok(ControlFlow::Return(value)),
                ControlFlow::Goto(label) => {
                    if let Some(target) = label_map.get(&label) {
                        index = *target;
                    } else {
                        return Ok(ControlFlow::Goto(label));
                    }
                }
            }
        }
        Ok(ControlFlow::Next)
    }

    fn exec_switch(&mut self, expr: &Expr, body: &Stmt) -> Result<ControlFlow, RuntimeError> {
        let value = self.eval_expr(expr)?.as_i64();
        let stmts = match body {
            Stmt::Compound(stmts) => stmts.as_slice(),
            _ => {
                return Err(RuntimeError::message(
                    "switch body must be compound statement",
                ))
            }
        };

        let mut default_index = None;
        let mut start_index = None;
        for (idx, stmt) in stmts.iter().enumerate() {
            match stmt {
                Stmt::Case(case_value, _) if *case_value == value => {
                    start_index = Some(idx);
                    break;
                }
                Stmt::Default(_) => {
                    if default_index.is_none() {
                        default_index = Some(idx);
                    }
                }
                _ => {}
            }
        }

        let mut index = match start_index.or(default_index) {
            Some(index) => index,
            None => return Ok(ControlFlow::Next),
        };

        while index < stmts.len() {
            let stmt = &stmts[index];
            let current = match stmt {
                Stmt::Case(_, inner) => inner.as_ref(),
                Stmt::Default(inner) => inner.as_ref(),
                _ => stmt,
            };
            match self.exec_stmt(current)? {
                ControlFlow::Next => index += 1,
                ControlFlow::Break => return Ok(ControlFlow::Next),
                ControlFlow::Return(value) => return Ok(ControlFlow::Return(value)),
                ControlFlow::Goto(label) => return Ok(ControlFlow::Goto(label)),
            }
        }
        Ok(ControlFlow::Next)
    }

    fn eval_expr(&mut self, expr: &Expr) -> Result<BValue, RuntimeError> {
        match expr {
            Expr::Constant(value) | Expr::CharConst(value) => Ok(BValue(*value)),
            Expr::StringLit(value) => {
                let addr = self.intern_string(value)?;
                Ok(BValue(addr))
            }
            Expr::Var(name) => {
                let addr = self.resolve_var_address(name)?;
                self.load_address(addr)
            }
            Expr::Unary(op, expr) => {
                let value = self.eval_expr(expr)?.as_i64();
                let result = match op {
                    UnaryOp::Neg => -value,
                    UnaryOp::Not => {
                        if value == 0 {
                            1
                        } else {
                            0
                        }
                    }
                    UnaryOp::BitNot => !value,
                };
                Ok(BValue(result))
            }
            Expr::Binary(op, left, right) => self.eval_binary(*op, left, right),
            Expr::Conditional(cond, then_expr, else_expr) => {
                let value = self.eval_expr(cond)?.as_i64();
                if value != 0 {
                    self.eval_expr(then_expr)
                } else {
                    self.eval_expr(else_expr)
                }
            }
            Expr::Assign(left, right) => {
                let addr = self.lvalue_address(left)?;
                let value = self.eval_expr(right)?;
                self.store_address(addr, value)?;
                Ok(value)
            }
            Expr::AssignOp(op, left, right) => {
                let addr = self.lvalue_address(left)?;
                let current = self.load_address(addr)?.as_i64();
                let rhs = self.eval_expr(right)?.as_i64();
                let value = self.apply_binary(*op, current, rhs)?;
                self.store_address(addr, BValue(value))?;
                Ok(BValue(value))
            }
            Expr::Call(name, args) => {
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    values.push(self.eval_expr(arg)?);
                }
                self.call_function(name, values)
            }
            Expr::Subscript(base, index) => {
                let base_addr = self.eval_expr(base)?.as_i64();
                let offset = self.eval_expr(index)?.as_i64();
                let addr = add_offset(base_addr, offset);
                self.load_address(addr)
            }
            Expr::AddressOf(expr) => {
                let addr = self.lvalue_address(expr)?;
                Ok(BValue(addr))
            }
            Expr::Indir(expr) => {
                let addr = self.eval_expr(expr)?.as_i64();
                self.load_address(addr)
            }
            Expr::Increment(expr, prefix) => {
                let addr = self.lvalue_address(expr)?;
                let value = self.load_address(addr)?.as_i64();
                let next = value + 1;
                self.store_address(addr, BValue(next))?;
                Ok(BValue(if *prefix { next } else { value }))
            }
            Expr::Decrement(expr, prefix) => {
                let addr = self.lvalue_address(expr)?;
                let value = self.load_address(addr)?.as_i64();
                let next = value - 1;
                self.store_address(addr, BValue(next))?;
                Ok(BValue(if *prefix { next } else { value }))
            }
        }
    }

    fn eval_binary(
        &mut self,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
    ) -> Result<BValue, RuntimeError> {
        match op {
            BinaryOp::And => {
                let left_val = self.eval_expr(left)?.as_i64();
                if left_val == 0 {
                    return Ok(BValue(0));
                }
                let right_val = self.eval_expr(right)?.as_i64();
                Ok(BValue(if right_val == 0 { 0 } else { 1 }))
            }
            BinaryOp::Or => {
                let left_val = self.eval_expr(left)?.as_i64();
                if left_val != 0 {
                    return Ok(BValue(1));
                }
                let right_val = self.eval_expr(right)?.as_i64();
                Ok(BValue(if right_val == 0 { 0 } else { 1 }))
            }
            _ => {
                let lhs = self.eval_expr(left)?.as_i64();
                let rhs = self.eval_expr(right)?.as_i64();
                Ok(BValue(self.apply_binary(op, lhs, rhs)?))
            }
        }
    }

    fn apply_binary(&self, op: BinaryOp, lhs: i64, rhs: i64) -> Result<i64, RuntimeError> {
        let result = match op {
            BinaryOp::Add => lhs + rhs,
            BinaryOp::Sub => lhs - rhs,
            BinaryOp::Mul => lhs * rhs,
            BinaryOp::Div => {
                if rhs == 0 {
                    return Err(RuntimeError::message("division by zero"));
                }
                lhs / rhs
            }
            BinaryOp::Rem => {
                if rhs == 0 {
                    return Err(RuntimeError::message("division by zero"));
                }
                lhs % rhs
            }
            BinaryOp::LShift => lhs << rhs,
            BinaryOp::RShift => lhs >> rhs,
            BinaryOp::Lt => {
                if lhs < rhs {
                    1
                } else {
                    0
                }
            }
            BinaryOp::Le => {
                if lhs <= rhs {
                    1
                } else {
                    0
                }
            }
            BinaryOp::Gt => {
                if lhs > rhs {
                    1
                } else {
                    0
                }
            }
            BinaryOp::Ge => {
                if lhs >= rhs {
                    1
                } else {
                    0
                }
            }
            BinaryOp::Eq => {
                if lhs == rhs {
                    1
                } else {
                    0
                }
            }
            BinaryOp::Ne => {
                if lhs != rhs {
                    1
                } else {
                    0
                }
            }
            BinaryOp::BitAnd => lhs & rhs,
            BinaryOp::BitXor => lhs ^ rhs,
            BinaryOp::BitOr => lhs | rhs,
            BinaryOp::And | BinaryOp::Or => unreachable!(),
        };
        Ok(result)
    }

    fn lvalue_address(&mut self, expr: &Expr) -> Result<i64, RuntimeError> {
        match expr {
            Expr::Var(name) => self.resolve_var_address(name),
            Expr::Subscript(base, index) => {
                let base_addr = self.eval_expr(base)?.as_i64();
                let offset = self.eval_expr(index)?.as_i64();
                Ok(add_offset(base_addr, offset))
            }
            Expr::Indir(inner) => Ok(self.eval_expr(inner)?.as_i64()),
            _ => Err(RuntimeError::message("invalid lvalue")),
        }
    }

    fn resolve_var_address(&mut self, name: &str) -> Result<i64, RuntimeError> {
        if let Some(frame) = self.frames.last() {
            if let Some(layout) = self.local_layouts.get(&frame.func) {
                if let Some(symbol) = layout.symbols.get(name) {
                    return Ok(encode_local(symbol.slot));
                }
            }
        }

        let slot = self.get_or_create_global(name);
        Ok(encode_global(slot))
    }

    fn get_or_create_global(&mut self, name: &str) -> usize {
        if let Some(symbol) = self.global_symbols.get(name) {
            return symbol.slot;
        }
        let slot = self.globals.allocate_block(1);
        self.global_symbols.insert(
            name.to_string(),
            GlobalSymbol {
                slot,
                is_vector: false,
                vector_base: None,
            },
        );
        slot
    }

    fn load_address(&mut self, addr: i64) -> Result<BValue, RuntimeError> {
        if addr < 0 {
            return Err(RuntimeError::message("invalid address"));
        }
        match decode_address(addr) {
            Address::Local(index) => {
                let frame = self
                    .frames
                    .last()
                    .ok_or_else(|| RuntimeError::message("no frame"))?;
                let value = frame
                    .locals
                    .get(index)
                    .ok_or_else(|| RuntimeError::message("local address out of range"))?;
                Ok(*value)
            }
            Address::Global(index) => {
                let value = self
                    .globals
                    .data
                    .get(index)
                    .ok_or_else(|| RuntimeError::message("global address out of range"))?;
                Ok(*value)
            }
        }
    }

    fn store_address(&mut self, addr: i64, value: BValue) -> Result<(), RuntimeError> {
        if addr < 0 {
            return Err(RuntimeError::message("invalid address"));
        }
        match decode_address(addr) {
            Address::Local(index) => {
                let frame = self
                    .frames
                    .last_mut()
                    .ok_or_else(|| RuntimeError::message("no frame"))?;
                let slot = frame
                    .locals
                    .get_mut(index)
                    .ok_or_else(|| RuntimeError::message("local address out of range"))?;
                *slot = value;
                Ok(())
            }
            Address::Global(index) => {
                let slot = self
                    .globals
                    .data
                    .get_mut(index)
                    .ok_or_else(|| RuntimeError::message("global address out of range"))?;
                *slot = value;
                Ok(())
            }
        }
    }

    fn intern_string(&mut self, value: &str) -> Result<i64, RuntimeError> {
        if let Some(addr) = self.string_pool.get(value) {
            return Ok(*addr);
        }

        let base = self.globals.allocate_block(value.len() + 1);
        for (idx, byte) in value.bytes().enumerate() {
            self.globals.data[base + idx] = BValue(byte as i64);
        }
        self.globals.data[base + value.len()] = BValue(STRING_TERMINATOR);
        let addr = encode_global(base);
        self.string_pool.insert(value.to_string(), addr);
        Ok(addr)
    }

    fn call_builtin(&mut self, name: &str, args: &[BValue]) -> Result<Option<BValue>, RuntimeError> {
        match name {
            "getchar" => {
                let mut buf = [0_u8; 1];
                let size = self
                    .input
                    .read(&mut buf)
                    .map_err(|err| RuntimeError::message(err.to_string()))?;
                let value = if size == 0 { -1 } else { buf[0] as i64 };
                Ok(Some(BValue(value)))
            }
            "putchar" => {
                let value = args.get(0).copied().unwrap_or(BValue(0));
                let ch = (value.as_i64() & 0xFF) as u8;
                self.output.write_all(&[ch])?;
                self.output.flush()?;
                Ok(Some(value))
            }
            "putnumbs" => {
                let value = args.get(0).copied().unwrap_or(BValue(0)).as_i64();
                self.output
                    .write_all(value.to_string().as_bytes())?;
                Ok(Some(BValue(value)))
            }
            "printf" => {
                let fmt_addr = args
                    .get(0)
                    .copied()
                    .ok_or_else(|| RuntimeError::message("printf requires format"))?
                    .as_i64();
                let fmt = self.read_c_string(fmt_addr)?;
                let mut output = Vec::new();
                let mut arg_index = 1;
                let mut iter = fmt.iter().copied();
                while let Some(ch) = iter.next() {
                    if ch != b'%' {
                        output.push(ch);
                        continue;
                    }

                    match iter.next() {
                        Some(b'%') => output.push(b'%'),
                        Some(b'd') => {
                            let value = args.get(arg_index).copied().unwrap_or(BValue(0));
                            arg_index += 1;
                            output.extend_from_slice(value.as_i64().to_string().as_bytes());
                        }
                        Some(b'o') => {
                            let value = args.get(arg_index).copied().unwrap_or(BValue(0));
                            arg_index += 1;
                            output.extend_from_slice(format!("{:o}", value.as_i64()).as_bytes());
                        }
                        Some(b'c') => {
                            let value = args.get(arg_index).copied().unwrap_or(BValue(0));
                            arg_index += 1;
                            output.push((value.as_i64() & 0xFF) as u8);
                        }
                        Some(b's') => {
                            let addr = args.get(arg_index).copied().unwrap_or(BValue(0));
                            arg_index += 1;
                            let bytes = self.read_c_string(addr.as_i64())?;
                            output.extend_from_slice(&bytes);
                        }
                        Some(other) => output.push(other),
                        None => break,
                    }
                }
                self.output.write_all(&output)?;
                Ok(Some(BValue(output.len() as i64)))
            }
            "char" => {
                let addr = args.get(0).copied().unwrap_or(BValue(0)).as_i64();
                let index = args.get(1).copied().unwrap_or(BValue(0)).as_i64();
                let value = self.load_address(add_offset(addr, index))?;
                Ok(Some(value))
            }
            "lchar" => {
                let addr = args.get(0).copied().unwrap_or(BValue(0)).as_i64();
                let index = args.get(1).copied().unwrap_or(BValue(0)).as_i64();
                let value = args.get(2).copied().unwrap_or(BValue(0));
                self.store_address(add_offset(addr, index), value)?;
                Ok(Some(value))
            }
            "getstr" => {
                let addr = args.get(0).copied().unwrap_or(BValue(0)).as_i64();
                let mut line = String::new();
                self.input
                    .read_line(&mut line)
                    .map_err(|err| RuntimeError::message(err.to_string()))?;
                while line.ends_with('\n') || line.ends_with('\r') {
                    line.pop();
                }
                for (idx, byte) in line.bytes().enumerate() {
                    self.store_address(add_offset(addr, idx as i64), BValue(byte as i64))?;
                }
                self.store_address(
                    add_offset(addr, line.len() as i64),
                    BValue(STRING_TERMINATOR),
                )?;
                Ok(Some(BValue(addr)))
            }
            "putstr" => {
                let addr = args.get(0).copied().unwrap_or(BValue(0)).as_i64();
                let bytes = self.read_c_string(addr)?;
                self.output.write_all(&bytes)?;
                Ok(Some(BValue(addr)))
            }
            "concat" => {
                let dest = args.get(0).copied().unwrap_or(BValue(0)).as_i64();
                let mut offset = 0;
                for arg in args.iter().skip(1) {
                    let addr = arg.as_i64();
                    let bytes = self.read_c_string(addr)?;
                    for byte in bytes {
                        self.store_address(add_offset(dest, offset), BValue(byte as i64))?;
                        offset += 1;
                    }
                }
                self.store_address(add_offset(dest, offset), BValue(STRING_TERMINATOR))?;
                Ok(Some(BValue(dest)))
            }
            "nargs" => {
                let count = self.frames.last().map(|f| f.nargs).unwrap_or(0);
                Ok(Some(BValue(count as i64)))
            }
            "exit" => Err(RuntimeError::Exit(0)),
            "openr" | "openw" | "flush" | "reread" | "system" | "getarg" | "getvec"
            | "rlsvec" => Ok(Some(BValue(0))),
            _ => Ok(None),
        }
    }

    fn read_c_string(&mut self, addr: i64) -> Result<Vec<u8>, RuntimeError> {
        let mut bytes = Vec::new();
        let mut offset = 0;
        loop {
            let value = self.load_address(add_offset(addr, offset))?.as_i64();
            if value == STRING_TERMINATOR {
                break;
            }
            bytes.push((value & 0xFF) as u8);
            offset += 1;
        }
        Ok(bytes)
    }
}

#[derive(Clone, Debug)]
enum ControlFlow {
    Next,
    Break,
    Return(BValue),
    Goto(String),
}
