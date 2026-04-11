/// B-LANG Phase 5 — Namespace-aware Library Registry
///
/// Each library produces a `Library` value that carries four maps:
///   - `math_namespaced`  : math builtins under `lib::fn` (e.g. `math::abs`)
///   - `math_bare`        : math builtins under bare names (e.g. `abs`)
///   - `string_namespaced`: string builtins under `lib::fn` (e.g. `string::strlen`)
///   - `string_bare`      : string builtins under bare names (activated by `use namespace`)
///
/// The interpreter merges these into its own registries in `with_io`,
/// honouring include guards and use-namespace ordering rules.
use crate::error::RuntimeError;
use crate::math::{register_math_builtins, MathFn};
use crate::string_lib::{StringBuiltinFn, STRING_BUILTINS};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Library struct
// ---------------------------------------------------------------------------

/// A resolved library.  Produced by `resolve_include` and consumed by the
/// interpreter's initialisation code.
pub struct Library {
    /// Math builtins accessible as `math::name` (always available after include).
    pub math_namespaced: HashMap<String, MathFn>,
    /// Math builtins accessible as bare `name`.
    /// For the math library these are always activated (Phase 4 compat).
    pub math_bare: HashMap<String, MathFn>,
    /// String builtins accessible as `string::name` (always available after include).
    pub string_namespaced: HashMap<String, StringBuiltinFn>,
    /// String builtins accessible as bare `name`.
    /// Only activated when `use namespace <lib>` is present.
    pub string_bare: HashMap<String, StringBuiltinFn>,
}

// ---------------------------------------------------------------------------
// Per-library builders
// ---------------------------------------------------------------------------

fn build_math_library() -> Library {
    // Collect bare names via the Phase 4 registration function.
    let mut bare: HashMap<String, MathFn> = HashMap::new();
    register_math_builtins(&mut bare);
    // Also expose under math:: prefix.
    let namespaced: HashMap<String, MathFn> = bare
        .iter()
        .map(|(name, &f)| (format!("math::{}", name), f))
        .collect();
    Library {
        math_namespaced: namespaced,
        math_bare: bare,
        string_namespaced: HashMap::new(),
        string_bare: HashMap::new(),
    }
}

fn build_string_library() -> Library {
    let string_namespaced: HashMap<String, StringBuiltinFn> = STRING_BUILTINS
        .iter()
        .map(|&(n, f)| (format!("string::{}", n), f))
        .collect();
    let string_bare: HashMap<String, StringBuiltinFn> = STRING_BUILTINS
        .iter()
        .map(|&(n, f)| (n.to_string(), f))
        .collect();
    Library {
        math_namespaced: HashMap::new(),
        math_bare: HashMap::new(),
        string_namespaced,
        string_bare,
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Look up library `name` and return its `Library` descriptor.
///
/// Returns `Ok(Library)` on success.
/// Returns `Err` (with an `[error]` message) if the library is unknown.
///
/// Calling `resolve_include` twice for the same name is safe — the caller
/// is responsible for include-guard logic (deduplication happens in
/// `Interpreter::with_io`).
pub fn resolve_include(name: &str) -> Result<Library, RuntimeError> {
    match name {
        "math" => Ok(build_math_library()),
        "string" => Ok(build_string_library()),
        unknown => Err(RuntimeError::message(format!(
            "[error] unknown library '{}'",
            unknown
        ))),
    }
}
