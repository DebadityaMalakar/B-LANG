/// B-LANG Phase 4 — Library Registry
///
/// Maps library names (as written after `include`) to their registration
/// functions.  This is the single extension point for adding future libraries
/// (Phase 5 Raylib, etc.) without touching anything else.
use crate::error::RuntimeError;
use crate::math::{register_math_builtins, MathFn};
use std::collections::HashMap;

/// Register all builtins from library `name` into `registry`.
///
/// Returns `Ok(())` on success.  Returns `Err` if the library name is not
/// recognised — the caller should surface this as a fatal error before calling
/// `main`.
///
/// Calling this more than once for the same library is allowed; the last
/// registration wins (identical function pointers, so functionally a no-op).
pub fn resolve_include(
    name: &str,
    registry: &mut HashMap<String, MathFn>,
) -> Result<(), RuntimeError> {
    match name {
        "math" => {
            register_math_builtins(registry);
            Ok(())
        }
        unknown => Err(RuntimeError::message(format!(
            "unknown library '{}'",
            unknown
        ))),
    }
}
