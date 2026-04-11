use crate::ast::{BinaryOp, Expr, Function, Program, Stmt, UnaryOp};
use crate::builtins::STRING_TERMINATOR;
use crate::error::RuntimeError;
use crate::libraries::resolve_include;
use crate::math::MathFn;
use crate::memory::{
    add_offset, decode_address, encode_global, encode_heap, encode_local, Address, BValue, Frame,
    GlobalMemory, Heap,
};
use crate::string_lib::StringBuiltinFn;
use crate::symbol::{GlobalSymbol, LocalLayout, LocalSymbol};
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, BufWriter, Read, Write};

/// Maximum call-stack depth before `StackOverflow` is raised.
/// Each B-language frame corresponds to one level of Rust recursion through
/// call_function → exec_stmt → eval_expr, so this value also guards against
/// native-stack overflow in the tree-walking interpreter.
const MAX_STACK_DEPTH: usize = 128;

// ---------------------------------------------------------------------------
// Output sink
// ---------------------------------------------------------------------------

pub enum OutputSink {
    Stdout(BufWriter<io::Stdout>),
    Buffer(Vec<u8>),
}

impl OutputSink {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
        match self {
            OutputSink::Stdout(writer) => writer
                .write_all(bytes)
                .map_err(|e| RuntimeError::message(e.to_string())),
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
                .map_err(|e| RuntimeError::message(e.to_string())),
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

// ---------------------------------------------------------------------------
// Interpreter
// ---------------------------------------------------------------------------

pub struct Interpreter {
    globals: GlobalMemory,
    heap: Heap,
    global_symbols: HashMap<String, GlobalSymbol>,
    functions: HashMap<String, Function>,
    local_layouts: HashMap<String, LocalLayout>,
    frames: Vec<Frame>,
    input: Box<dyn BufRead>,
    output: OutputSink,
    string_pool: HashMap<String, i64>,
    /// Registry of math builtins (bare + prefixed) populated by `include math`.
    math_registry: HashMap<String, MathFn>,
    /// Registry of string builtins populated by `include string`.
    /// Prefixed names (`string::fn`) are always present after include.
    /// Bare names (`fn`) are added when `use namespace string` is declared.
    string_registry: HashMap<String, StringBuiltinFn>,
    /// Deferred error from a failed `include` / `use namespace`; surfaced in run_main.
    include_error: Option<RuntimeError>,
    /// Emit heap/stack dumps and memory access warnings.
    debug_memory: bool,
    /// Trace every function call, return, and goto to stderr.
    trace_exec: bool,
    /// Log every memory read/write to stderr (pointer tracing).
    strict_memory: bool,
    /// Warn to stderr on math domain errors (sqrt(-1), ln(0), etc.).
    strict_math: bool,
}

impl Interpreter {
    pub fn new(program: Program) -> Self {
        let input = Box::new(io::BufReader::new(io::stdin()));
        let output = OutputSink::Stdout(BufWriter::new(io::stdout()));
        Self::with_io(program, input, output)
    }

    pub fn with_io(program: Program, input: Box<dyn BufRead>, output: OutputSink) -> Self {
        let (globals, heap, global_symbols) = Self::init_globals(&program);
        let functions = program
            .functions
            .iter()
            .cloned()
            .map(|f| (f.name.clone(), f))
            .collect();
        let local_layouts = program
            .functions
            .iter()
            .map(|f| (f.name.clone(), Self::build_local_layout(f)))
            .collect();

        // ── Phase 5 include + use-namespace resolution ────────────────────
        //
        // Processing order:
        //   1. Walk `program.includes` in source order.
        //      - Skip duplicates (include guard).
        //      - Detect circular includes (forward-looking infrastructure;
        //        built-in libs cannot include each other, so this never fires
        //        today but the Vec is in place for user-defined libs later).
        //      - Merge math maps into math_registry (first-seen wins for bare names).
        //      - Add string::* names to string_registry immediately.
        //      - Store string bare-name maps in string_bare_by_lib for step 2.
        //   2. Walk `program.use_namespaces` in source order.
        //      - Validate that the named library was included; error if not.
        //      - Activate bare names into string_registry (first-seen wins).
        //
        // Any error is captured in include_error and surfaced by run_main
        // before executing the B program.

        let mut math_registry: HashMap<String, MathFn> = HashMap::new();
        let mut string_registry: HashMap<String, StringBuiltinFn> = HashMap::new();
        // Bare string maps saved per library name for later use-namespace activation.
        let mut string_bare_by_lib: HashMap<String, HashMap<String, StringBuiltinFn>> =
            HashMap::new();
        let mut included: HashSet<String> = HashSet::new();
        let mut include_stack: Vec<String> = Vec::new();
        let mut include_error: Option<RuntimeError> = None;

        for lib_name in &program.includes {
            // Include guard: skip silently if already registered.
            if included.contains(lib_name) {
                continue;
            }
            // Circular detection: should never fire for built-in libs, but
            // the infrastructure must exist for Phase 6+ user libs.
            if include_stack.contains(lib_name) {
                let cycle = format!("{} -> {}", include_stack.join(" -> "), lib_name);
                eprintln!("[warning] circular include detected: {}", cycle);
                include_error = Some(RuntimeError::message(format!(
                    "[warning] circular include detected: {}",
                    cycle
                )));
                break;
            }
            include_stack.push(lib_name.clone());
            match resolve_include(lib_name) {
                Ok(library) => {
                    // Math: bare names always active (Phase 4 compat); first-seen wins.
                    for (name, f) in &library.math_bare {
                        math_registry.entry(name.clone()).or_insert(*f);
                    }
                    // Math: namespaced names (math::fn) always active.
                    for (name, f) in &library.math_namespaced {
                        math_registry.entry(name.clone()).or_insert(*f);
                    }
                    // String: prefixed names always active after include.
                    for (name, f) in &library.string_namespaced {
                        string_registry.entry(name.clone()).or_insert(*f);
                    }
                    // String: save bare map for use-namespace activation later.
                    if !library.string_bare.is_empty() {
                        string_bare_by_lib
                            .insert(lib_name.clone(), library.string_bare);
                    }
                    included.insert(lib_name.clone());
                }
                Err(e) => {
                    include_error = Some(e);
                    break;
                }
            }
            include_stack.pop();
        }

        // Step 2: activate bare names for each declared namespace.
        if include_error.is_none() {
            for ns_name in &program.use_namespaces {
                if !included.contains(ns_name) {
                    let msg = format!("[error] '{}' has not been included", ns_name);
                    eprintln!("{}", msg);
                    include_error = Some(RuntimeError::message(msg));
                    break;
                }
                if let Some(bare_map) = string_bare_by_lib.get(ns_name) {
                    for (name, f) in bare_map {
                        // First-include-wins: don't overwrite a name already
                        // activated by an earlier use namespace statement.
                        string_registry.entry(name.clone()).or_insert(*f);
                    }
                }
            }
        }

        Self {
            globals,
            heap,
            global_symbols,
            functions,
            local_layouts,
            frames: Vec::new(),
            input,
            output,
            string_pool: HashMap::new(),
            math_registry,
            string_registry,
            include_error,
            debug_memory: false,
            trace_exec: false,
            strict_memory: false,
            strict_math: false,
        }
    }

    // -- configuration -------------------------------------------------------

    pub fn set_output(&mut self, output: OutputSink) {
        self.output = output;
    }

    pub fn set_debug_memory(&mut self, enabled: bool) {
        self.debug_memory = enabled;
    }

    pub fn set_trace_exec(&mut self, enabled: bool) {
        self.trace_exec = enabled;
    }

    pub fn set_strict_memory(&mut self, enabled: bool) {
        self.strict_memory = enabled;
    }

    pub fn set_strict_math(&mut self, enabled: bool) {
        self.strict_math = enabled;
    }

    // -- entry point ---------------------------------------------------------

    pub fn run_main(&mut self) -> Result<BValue, RuntimeError> {
        // Surface any include-resolution error before executing main.
        if let Some(err) = self.include_error.take() {
            return Err(err);
        }
        match self.call_function("main", Vec::new()) {
            Ok(value) => Ok(value),
            Err(RuntimeError::Exit(code)) => Ok(BValue(code)),
            Err(err) => Err(err),
        }
    }

    pub fn take_output(&mut self) -> Option<Vec<u8>> {
        self.output.take_buffer()
    }

    // -------------------------------------------------------------------------
    // Debug utilities
    // -------------------------------------------------------------------------

    pub fn dump_heap(&self) {
        eprintln!("=== HEAP DUMP ({} slots) ===", self.heap.data.len());
        for (i, val) in self.heap.data.iter().enumerate() {
            let ch = if val.0 >= 0x20 && val.0 <= 0x7e {
                val.0 as u8 as char
            } else {
                '.'
            };
            eprintln!("  heap[{:4}] = {:10}  '{}'", i, val.0, ch);
        }
    }

    pub fn dump_stack(&self) {
        eprintln!("=== STACK DUMP ({} frames) ===", self.frames.len());
        for (fi, frame) in self.frames.iter().enumerate() {
            eprintln!(
                "  frame[{}] {} (nargs={}, bp={}, {} locals):",
                fi,
                frame.func,
                frame.nargs,
                frame.base_pointer,
                frame.locals.len()
            );
            for (i, val) in frame.locals.iter().enumerate() {
                eprintln!("    local[{:3}] = {}", i, val.0);
            }
        }
    }

    fn trace(&self, msg: &str) {
        eprintln!("[TRACE] {}", msg);
    }

    fn mem_trace(&self, op: &str, addr: i64, val: BValue) {
        let decoded = match decode_address(addr) {
            Address::Global(i) => format!("global[{}]", i),
            Address::Local(i) => format!("local[{}]", i),
            Address::Heap(i) => format!("heap[{}]", i),
        };
        let ch = if val.0 >= 0x20 && val.0 <= 0x7e {
            format!(" '{}'", val.0 as u8 as char)
        } else {
            String::new()
        };
        eprintln!("[MEM] {} addr=0x{:x} ({}) = {}{}", op, addr, decoded, val.0, ch);
    }

    // -------------------------------------------------------------------------
    // Initialization
    // -------------------------------------------------------------------------

    fn init_globals(program: &Program) -> (GlobalMemory, Heap, HashMap<String, GlobalSymbol>) {
        let mut globals = GlobalMemory::new();
        let mut heap = Heap::new();
        let mut symbols = HashMap::new();

        for decl in &program.globals {
            let slot = globals.allocate_block(1);
            let mut symbol = GlobalSymbol {
                slot,
                is_vector: decl.vector_size.is_some(),
                vector_base: None,
            };
            if let Some(size) = decl.vector_size {
                let base = heap.allocate(size + 1);
                globals.data[slot] = BValue(encode_heap(base));
                symbol.vector_base = Some(base);
            }
            symbols.insert(decl.name.clone(), symbol);
        }

        (globals, heap, symbols)
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
                    vector_size: None,
                },
            );
            total_slots += 1;
        }

        for local in &function.locals {
            let slot = total_slots;
            total_slots += 1;
            let (is_vector, vector_size) = match local.vector_size {
                Some(size) => (true, Some(size)),
                None => (false, None),
            };
            symbols.insert(
                local.name.clone(),
                LocalSymbol {
                    slot,
                    is_vector,
                    vector_size,
                },
            );
        }

        LocalLayout {
            symbols,
            total_slots,
        }
    }

    // -------------------------------------------------------------------------
    // Function calls
    // -------------------------------------------------------------------------

    fn call_function(&mut self, name: &str, args: Vec<BValue>) -> Result<BValue, RuntimeError> {
        // Check stack depth before pushing a new frame.
        if self.frames.len() >= MAX_STACK_DEPTH {
            return Err(RuntimeError::StackOverflow);
        }

        if let Some(value) = self.call_builtin(name, &args)? {
            return Ok(value);
        }

        let function = self
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| RuntimeError::message(format!("undefined function '{}'", name)))?;
        let layout = self
            .local_layouts
            .get(name)
            .ok_or_else(|| RuntimeError::message("missing layout"))?
            .clone();

        let base_pointer = self.frames.len();
        let mut frame = Frame {
            func: name.to_string(),
            locals: vec![BValue(0); layout.total_slots],
            nargs: args.len(),
            base_pointer,
        };

        // Bind parameters.
        for (idx, param) in function.params.iter().enumerate() {
            if let Some(symbol) = layout.symbols.get(param) {
                frame.locals[symbol.slot] = args.get(idx).copied().unwrap_or(BValue(0));
            }
        }

        // Allocate auto vectors in the heap; store heap address in the local slot.
        for symbol in layout.symbols.values() {
            if symbol.is_vector {
                let size = symbol.vector_size.unwrap_or(0);
                let base = self.heap.allocate(size + 1);
                frame.locals[symbol.slot] = BValue(encode_heap(base));
            }
        }

        if self.trace_exec {
            self.trace(&format!("CALL {}({} args)", name, args.len()));
        }
        if self.debug_memory {
            eprintln!(
                "[DEBUG] call {} (nargs={}, frame_depth={})",
                name,
                args.len(),
                base_pointer
            );
        }

        self.frames.push(frame);
        let flow = self.exec_stmt(&function.body);
        self.frames.pop();

        let result = match flow? {
            ControlFlow::Return(value) => Ok(value),
            ControlFlow::Next => Ok(BValue(0)),
            ControlFlow::Break => Ok(BValue(0)),
            ControlFlow::Goto(label) => Err(RuntimeError::InvalidGoto(label)),
        };

        if self.trace_exec {
            let val_str = match &result {
                Ok(v) => v.0.to_string(),
                Err(e) => format!("ERR({})", e),
            };
            self.trace(&format!("RETURN {} (from {})", val_str, name));
        }

        result
    }

    // -------------------------------------------------------------------------
    // Statement execution
    // -------------------------------------------------------------------------

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
            Stmt::Goto(label) => {
                if self.trace_exec {
                    self.trace(&format!("GOTO {}", label));
                }
                Ok(ControlFlow::Goto(label.clone()))
            }
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
        // Pre-scan labels so goto can resolve forward references within this block.
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
                    if let Some(&target) = label_map.get(&label) {
                        if self.trace_exec {
                            self.trace(&format!("RESOLVED goto {} -> stmt[{}]", label, target));
                        }
                        index = target;
                    } else {
                        // Propagate to an outer compound block or function handler.
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
                    "switch body must be a compound statement",
                ))
            }
        };

        // Linear scan for the matching case or default.
        let mut default_index = None;
        let mut start_index = None;
        for (idx, stmt) in stmts.iter().enumerate() {
            match stmt {
                Stmt::Case(case_value, _) if *case_value == value => {
                    start_index = Some(idx);
                    break;
                }
                Stmt::Default(_) if default_index.is_none() => {
                    default_index = Some(idx);
                }
                _ => {}
            }
        }

        let mut index = match start_index.or(default_index) {
            Some(idx) => idx,
            None => return Ok(ControlFlow::Next),
        };

        // Execute from the matched position, falling through until break/end.
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
                ControlFlow::Return(v) => return Ok(ControlFlow::Return(v)),
                ControlFlow::Goto(label) => return Ok(ControlFlow::Goto(label)),
            }
        }
        Ok(ControlFlow::Next)
    }

    // -------------------------------------------------------------------------
    // Expression evaluation
    // -------------------------------------------------------------------------

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
                    UnaryOp::Not => (value == 0) as i64,
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
            // Short-circuit operators.
            BinaryOp::And => {
                let lv = self.eval_expr(left)?.as_i64();
                if lv == 0 {
                    return Ok(BValue(0));
                }
                let rv = self.eval_expr(right)?.as_i64();
                Ok(BValue((rv != 0) as i64))
            }
            BinaryOp::Or => {
                let lv = self.eval_expr(left)?.as_i64();
                if lv != 0 {
                    return Ok(BValue(1));
                }
                let rv = self.eval_expr(right)?.as_i64();
                Ok(BValue((rv != 0) as i64))
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
                    return Err(RuntimeError::DivisionByZero);
                }
                lhs / rhs
            }
            BinaryOp::Rem => {
                if rhs == 0 {
                    return Err(RuntimeError::DivisionByZero);
                }
                lhs % rhs
            }
            BinaryOp::LShift => lhs << (rhs & 63),
            BinaryOp::RShift => lhs >> (rhs & 63),
            BinaryOp::Lt => (lhs < rhs) as i64,
            BinaryOp::Le => (lhs <= rhs) as i64,
            BinaryOp::Gt => (lhs > rhs) as i64,
            BinaryOp::Ge => (lhs >= rhs) as i64,
            BinaryOp::Eq => (lhs == rhs) as i64,
            BinaryOp::Ne => (lhs != rhs) as i64,
            BinaryOp::BitAnd => lhs & rhs,
            BinaryOp::BitXor => lhs ^ rhs,
            BinaryOp::BitOr => lhs | rhs,
            BinaryOp::And | BinaryOp::Or => unreachable!(),
        };
        Ok(result)
    }

    // -------------------------------------------------------------------------
    // Address helpers
    // -------------------------------------------------------------------------

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
        // Local symbols take priority over globals.
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

    // -------------------------------------------------------------------------
    // Unified memory read / write
    // -------------------------------------------------------------------------

    fn load_address(&mut self, addr: i64) -> Result<BValue, RuntimeError> {
        if addr < 0 {
            return Err(RuntimeError::InvalidMemoryAccess(addr));
        }
        let value = match decode_address(addr) {
            Address::Local(index) => {
                let frame = self
                    .frames
                    .last()
                    .ok_or_else(|| RuntimeError::message("load from local with no frame"))?;
                frame
                    .locals
                    .get(index)
                    .copied()
                    .ok_or(RuntimeError::InvalidMemoryAccess(addr))?
            }
            Address::Heap(index) => self
                .heap
                .data
                .get(index)
                .copied()
                .ok_or(RuntimeError::InvalidMemoryAccess(addr))?,
            Address::Global(index) => self
                .globals
                .data
                .get(index)
                .copied()
                .ok_or(RuntimeError::InvalidMemoryAccess(addr))?,
        };
        if self.strict_memory {
            self.mem_trace("READ ", addr, value);
        }
        Ok(value)
    }

    fn store_address(&mut self, addr: i64, value: BValue) -> Result<(), RuntimeError> {
        if addr < 0 {
            return Err(RuntimeError::InvalidMemoryAccess(addr));
        }
        if self.strict_memory {
            self.mem_trace("WRITE", addr, value);
        }
        match decode_address(addr) {
            Address::Local(index) => {
                let frame = self
                    .frames
                    .last_mut()
                    .ok_or_else(|| RuntimeError::message("store to local with no frame"))?;
                let slot = frame
                    .locals
                    .get_mut(index)
                    .ok_or(RuntimeError::InvalidMemoryAccess(addr))?;
                *slot = value;
                Ok(())
            }
            Address::Heap(index) => {
                let slot = self
                    .heap
                    .data
                    .get_mut(index)
                    .ok_or(RuntimeError::InvalidMemoryAccess(addr))?;
                *slot = value;
                Ok(())
            }
            Address::Global(index) => {
                let slot = self
                    .globals
                    .data
                    .get_mut(index)
                    .ok_or(RuntimeError::InvalidMemoryAccess(addr))?;
                *slot = value;
                Ok(())
            }
        }
    }

    // -------------------------------------------------------------------------
    // String interning — literals are placed in the heap at parse time.
    // -------------------------------------------------------------------------

    fn intern_string(&mut self, value: &str) -> Result<i64, RuntimeError> {
        if let Some(&addr) = self.string_pool.get(value) {
            return Ok(addr);
        }
        let base = self.heap.allocate(value.len() + 1);
        for (idx, byte) in value.bytes().enumerate() {
            self.heap.data[base + idx] = BValue(byte as i64);
        }
        self.heap.data[base + value.len()] = BValue(STRING_TERMINATOR);
        let addr = encode_heap(base);
        self.string_pool.insert(value.to_string(), addr);
        Ok(addr)
    }

    // -------------------------------------------------------------------------
    // Built-in functions
    //
    // All builtins share the interface:
    //   fn(&mut Interpreter, &[BValue]) -> Result<Option<BValue>, RuntimeError>
    //
    // Returning Ok(None) means "not a builtin" — the caller will then look for
    // a user-defined function with that name.
    // -------------------------------------------------------------------------

    fn call_builtin(
        &mut self,
        name: &str,
        args: &[BValue],
    ) -> Result<Option<BValue>, RuntimeError> {
        // Math registry: copy pointer to avoid borrow conflicts.
        if let Some(math_fn) = self.math_registry.get(name).copied() {
            return Ok(Some(math_fn(args, self.strict_math)));
        }
        // String registry: string builtins need access to the raw heap data.
        if let Some(str_fn) = self.string_registry.get(name).copied() {
            return Ok(Some(str_fn(args, &mut self.heap.data, self.strict_math)));
        }

        match name {
            "getchar" => {
                let mut buf = [0_u8; 1];
                let n = self
                    .input
                    .read(&mut buf)
                    .map_err(|e| RuntimeError::message(e.to_string()))?;
                let value = if n == 0 { -1 } else { buf[0] as i64 };
                Ok(Some(BValue(value)))
            }

            "putchar" => {
                let ch = (args.first().copied().unwrap_or(BValue(0)).as_i64() & 0xFF) as u8;
                self.output.write_all(&[ch])?;
                self.output.flush()?;
                Ok(Some(BValue(ch as i64)))
            }

            "putnumbs" => {
                let n = args.first().copied().unwrap_or(BValue(0)).as_i64();
                self.output.write_all(n.to_string().as_bytes())?;
                Ok(Some(BValue(n)))
            }

            "printf" => {
                let fmt_addr = args
                    .first()
                    .copied()
                    .ok_or_else(|| RuntimeError::message("printf: missing format argument"))?
                    .as_i64();
                let fmt = self.read_c_string(fmt_addr)?;
                let mut out = Vec::new();
                let mut arg_idx = 1usize;
                let mut iter = fmt.iter().copied();
                while let Some(ch) = iter.next() {
                    if ch != b'%' {
                        out.push(ch);
                        continue;
                    }
                    match iter.next() {
                        Some(b'%') => out.push(b'%'),
                        Some(b'd') => {
                            let v = args.get(arg_idx).copied().unwrap_or(BValue(0));
                            arg_idx += 1;
                            out.extend_from_slice(v.as_i64().to_string().as_bytes());
                        }
                        Some(b'o') => {
                            let v = args.get(arg_idx).copied().unwrap_or(BValue(0));
                            arg_idx += 1;
                            out.extend_from_slice(format!("{:o}", v.as_i64()).as_bytes());
                        }
                        Some(b'c') => {
                            let v = args.get(arg_idx).copied().unwrap_or(BValue(0));
                            arg_idx += 1;
                            out.push((v.as_i64() & 0xFF) as u8);
                        }
                        Some(b's') => {
                            let addr = args.get(arg_idx).copied().unwrap_or(BValue(0));
                            arg_idx += 1;
                            let bytes = self.read_c_string(addr.as_i64())?;
                            out.extend_from_slice(&bytes);
                        }
                        Some(other) => {
                            out.push(b'%');
                            out.push(other);
                        }
                        None => break,
                    }
                }
                self.output.write_all(&out)?;
                Ok(Some(BValue(out.len() as i64)))
            }

            "char" => {
                let addr = args.first().copied().unwrap_or(BValue(0)).as_i64();
                let idx = args.get(1).copied().unwrap_or(BValue(0)).as_i64();
                let value = self.load_address(add_offset(addr, idx))?;
                Ok(Some(value))
            }

            "lchar" => {
                let addr = args.first().copied().unwrap_or(BValue(0)).as_i64();
                let idx = args.get(1).copied().unwrap_or(BValue(0)).as_i64();
                let value = args.get(2).copied().unwrap_or(BValue(0));
                self.store_address(add_offset(addr, idx), value)?;
                Ok(Some(value))
            }

            "getstr" => {
                let addr = args.first().copied().unwrap_or(BValue(0)).as_i64();
                let mut line = String::new();
                self.input
                    .read_line(&mut line)
                    .map_err(|e| RuntimeError::message(e.to_string()))?;
                while line.ends_with('\n') || line.ends_with('\r') {
                    line.pop();
                }
                for (i, byte) in line.bytes().enumerate() {
                    self.store_address(add_offset(addr, i as i64), BValue(byte as i64))?;
                }
                self.store_address(
                    add_offset(addr, line.len() as i64),
                    BValue(STRING_TERMINATOR),
                )?;
                Ok(Some(BValue(addr)))
            }

            "putstr" => {
                let addr = args.first().copied().unwrap_or(BValue(0)).as_i64();
                let bytes = self.read_c_string(addr)?;
                self.output.write_all(&bytes)?;
                Ok(Some(BValue(addr)))
            }

            "concat" => {
                let dest = args.first().copied().unwrap_or(BValue(0)).as_i64();
                let mut offset = 0i64;
                for arg in args.iter().skip(1) {
                    let src_addr = arg.as_i64();
                    let bytes = self.read_c_string(src_addr)?;
                    for byte in bytes {
                        self.store_address(add_offset(dest, offset), BValue(byte as i64))?;
                        offset += 1;
                    }
                }
                self.store_address(add_offset(dest, offset), BValue(STRING_TERMINATOR))?;
                Ok(Some(BValue(dest)))
            }

            "getvec" => {
                let size = args.first().copied().unwrap_or(BValue(0)).as_i64() as usize;
                let base = self.heap.allocate(size + 1);
                Ok(Some(BValue(encode_heap(base))))
            }

            "rlsvec" => {
                // Bump allocator — release is a no-op for MVP.
                Ok(Some(BValue(0)))
            }

            "nargs" => {
                let count = self.frames.last().map(|f| f.nargs).unwrap_or(0);
                Ok(Some(BValue(count as i64)))
            }

            "exit" => {
                let code = args.first().copied().unwrap_or(BValue(0)).as_i64();
                Err(RuntimeError::Exit(code))
            }

            // Stubbed I/O / system functions — return 0 silently.
            "openr" | "openw" | "flush" | "reread" | "system" | "getarg" => {
                Ok(Some(BValue(0)))
            }

            _ => Ok(None),
        }
    }

    /// Walk a B-string (terminated by `STRING_TERMINATOR`) and collect raw bytes.
    fn read_c_string(&mut self, addr: i64) -> Result<Vec<u8>, RuntimeError> {
        let mut bytes = Vec::new();
        let mut offset = 0i64;
        loop {
            let value = self.load_address(add_offset(addr, offset))?.as_i64();
            if value == STRING_TERMINATOR {
                break;
            }
            bytes.push((value & 0xFF) as u8);
            offset += 1;
            if offset > 65536 {
                return Err(RuntimeError::message("string read exceeded safety limit"));
            }
        }
        Ok(bytes)
    }
}

// ---------------------------------------------------------------------------
// Internal control-flow signal
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum ControlFlow {
    Next,
    Break,
    Return(BValue),
    Goto(String),
}
