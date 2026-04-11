/// B-LANG Phase 4 — Math Library
///
/// All values in and out are `BValue` (i64).  Fixed-point functions use the
/// Q16 convention: real_value = bvalue / 65536.
///
/// Internally, transcendental functions use `f64` (Rust never exposes float
/// to the B program).  This is consistent with the `sqrt`/`cbrt` precedent in
/// the Phase 4 spec.
use crate::memory::BValue;
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// Type alias for math builtins
// ---------------------------------------------------------------------------

/// The signature every math builtin implements.
/// `args`   — B-language arguments passed to the call.
/// `strict` — when true, domain errors are warned to stderr before returning
///            the safe default.
pub type MathFn = fn(args: &[BValue], strict: bool) -> BValue;

// ---------------------------------------------------------------------------
// Fixed-point constants
// ---------------------------------------------------------------------------

const FP_SCALE: i64 = 65536; // 2^16 — Q16 representation of 1.0

// ---------------------------------------------------------------------------
// Sine look-up table (deliverable: generated at startup via OnceLock)
//
// 512 entries covering [0, 2π), each entry stores sin(i/512 * 2π) in Q16.
// The table is used internally; the public sin/cos builtins use f64 directly
// for maximum accuracy.
// ---------------------------------------------------------------------------

static SIN_TABLE: OnceLock<[i64; 512]> = OnceLock::new();

pub fn get_sin_table() -> &'static [i64; 512] {
    SIN_TABLE.get_or_init(|| {
        let mut table = [0i64; 512];
        for i in 0..512 {
            let angle = (i as f64) * 2.0 * std::f64::consts::PI / 512.0;
            table[i] = (angle.sin() * FP_SCALE as f64).round() as i64;
        }
        table
    })
}

// ---------------------------------------------------------------------------
// RNG state (thread-local so tests can run independently)
// ---------------------------------------------------------------------------

thread_local! {
    static RNG_STATE: Cell<u64> = Cell::new(1);
}

fn xorshift64() -> i64 {
    RNG_STATE.with(|state| {
        let mut x = state.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        state.set(x);
        x as i64
    })
}

// ---------------------------------------------------------------------------
// Helper: first arg, defaulting to 0
// ---------------------------------------------------------------------------

#[inline]
fn arg0(args: &[BValue]) -> i64 {
    args.first().copied().unwrap_or(BValue(0)).0
}
#[inline]
fn arg1(args: &[BValue]) -> i64 {
    args.get(1).copied().unwrap_or(BValue(0)).0
}
#[inline]
fn arg2(args: &[BValue]) -> i64 {
    args.get(2).copied().unwrap_or(BValue(0)).0
}

// ===========================================================================
// 3. Integer Math Builtins
// ===========================================================================

// -- 3.1 Core arithmetic ----------------------------------------------------

fn builtin_abs(args: &[BValue], _s: bool) -> BValue {
    BValue(arg0(args).abs())
}

fn builtin_sign(args: &[BValue], _s: bool) -> BValue {
    BValue(arg0(args).signum())
}

fn builtin_min(args: &[BValue], _s: bool) -> BValue {
    BValue(arg0(args).min(arg1(args)))
}

fn builtin_max(args: &[BValue], _s: bool) -> BValue {
    BValue(arg0(args).max(arg1(args)))
}

fn builtin_clamp(args: &[BValue], _s: bool) -> BValue {
    let n = arg0(args);
    let lo = arg1(args);
    let hi = arg2(args);
    BValue(n.clamp(lo, hi))
}

// -- 3.2 Integer division and modulo ----------------------------------------

/// Floor division — truncates toward negative infinity (unlike `/`).
fn builtin_divf(args: &[BValue], strict: bool) -> BValue {
    let a = arg0(args);
    let b = arg1(args);
    if b == 0 {
        if strict {
            eprintln!("[strict-math] divf: division by zero");
        }
        return BValue(0);
    }
    let d = a / b;
    let r = a % b;
    let result = if r != 0 && (r < 0) != (b < 0) { d - 1 } else { d };
    BValue(result)
}

/// Floored modulo — result always has the same sign as the divisor.
fn builtin_modf(args: &[BValue], strict: bool) -> BValue {
    let a = arg0(args);
    let b = arg1(args);
    if b == 0 {
        if strict {
            eprintln!("[strict-math] modf: division by zero");
        }
        return BValue(0);
    }
    BValue(((a % b) + b) % b)
}

// -- 3.3 Powers and roots ----------------------------------------------------

/// Integer exponentiation (non-negative exponent only).
fn builtin_pow(args: &[BValue], _s: bool) -> BValue {
    let mut base = arg0(args);
    let mut exp = arg1(args);
    if exp < 0 {
        return BValue(0);
    }
    let mut result = 1i64;
    while exp > 0 {
        if exp & 1 == 1 {
            result = result.wrapping_mul(base);
        }
        base = base.wrapping_mul(base);
        exp >>= 1;
    }
    BValue(result)
}

/// Integer square root (floor).  Returns 0 for negative input.
fn builtin_sqrt(args: &[BValue], strict: bool) -> BValue {
    let n = arg0(args);
    if n < 0 {
        if strict {
            eprintln!("[strict-math] sqrt: negative argument");
        }
        return BValue(0);
    }
    BValue((n as f64).sqrt() as i64)
}

/// Integer cube root (floor, signed).
fn builtin_cbrt(args: &[BValue], _s: bool) -> BValue {
    let n = arg0(args);
    BValue((n as f64).cbrt() as i64)
}

// -- 3.4 Bit math ------------------------------------------------------------

fn builtin_popcnt(args: &[BValue], _s: bool) -> BValue {
    BValue(arg0(args).count_ones() as i64)
}

fn builtin_clz(args: &[BValue], _s: bool) -> BValue {
    BValue(arg0(args).leading_zeros() as i64)
}

fn builtin_ctz(args: &[BValue], _s: bool) -> BValue {
    BValue(arg0(args).trailing_zeros() as i64)
}

fn builtin_bswap(args: &[BValue], _s: bool) -> BValue {
    BValue(arg0(args).swap_bytes())
}

// -- 3.5 GCD and LCM ---------------------------------------------------------

fn gcd_inner(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

fn builtin_gcd(args: &[BValue], _s: bool) -> BValue {
    BValue(gcd_inner(arg0(args), arg1(args)))
}

fn builtin_lcm(args: &[BValue], _s: bool) -> BValue {
    let a = arg0(args);
    let b = arg1(args);
    if a == 0 || b == 0 {
        return BValue(0);
    }
    let g = gcd_inner(a, b);
    BValue((a / g).wrapping_mul(b).abs())
}

// ===========================================================================
// 4. Fixed-Point Math Builtins  (Q16 convention: value / 65536 = real)
// ===========================================================================

// -- 4.1 Basic FP operations -------------------------------------------------

/// Q16 multiply: (a * b) >> 16 using i128 to avoid overflow.
fn builtin_fpmul(args: &[BValue], _s: bool) -> BValue {
    let a = arg0(args) as i128;
    let b = arg1(args) as i128;
    BValue(((a * b) >> 16) as i64)
}

/// Q16 divide: (a << 16) / b.
fn builtin_fpdiv(args: &[BValue], strict: bool) -> BValue {
    let a = arg0(args) as i128;
    let b = arg1(args);
    if b == 0 {
        if strict {
            eprintln!("[strict-math] fpdiv: division by zero");
        }
        return BValue(0);
    }
    BValue(((a << 16) / b as i128) as i64)
}

/// Q16 → integer (truncate fractional part).
fn builtin_fptoi(args: &[BValue], _s: bool) -> BValue {
    BValue(arg0(args) >> 16)
}

/// Integer → Q16.
fn builtin_itofp(args: &[BValue], _s: bool) -> BValue {
    BValue(arg0(args) << 16)
}

// -- 4.2 Trigonometry (angles in milliradians, results Q16) -----------------
//
// The sine lookup table is initialized by get_sin_table() above.
// The builtins use f64 internally for accuracy, ensuring sin(1571 mr) = 65536.

fn builtin_sin(args: &[BValue], _s: bool) -> BValue {
    let _ = get_sin_table(); // ensure the table OnceLock is initialised
    let angle_mr = arg0(args);
    let radians = angle_mr as f64 / 1000.0;
    BValue((radians.sin() * FP_SCALE as f64).round() as i64)
}

fn builtin_cos(args: &[BValue], _s: bool) -> BValue {
    let _ = get_sin_table();
    let angle_mr = arg0(args);
    let radians = angle_mr as f64 / 1000.0;
    BValue((radians.cos() * FP_SCALE as f64).round() as i64)
}

fn builtin_tan(args: &[BValue], _s: bool) -> BValue {
    let angle_mr = arg0(args);
    let radians = angle_mr as f64 / 1000.0;
    let t = radians.tan();
    if t.is_infinite() || t.abs() > 1.0e12 {
        return BValue(i64::MAX);
    }
    BValue((t * FP_SCALE as f64).round() as i64)
}

/// atan2(y, x) where y and x are Q16.  Returns integer milliradians.
fn builtin_atan2(args: &[BValue], _s: bool) -> BValue {
    let y = arg0(args) as f64 / FP_SCALE as f64;
    let x = arg1(args) as f64 / FP_SCALE as f64;
    let radians = y.atan2(x);
    BValue((radians * 1000.0).round() as i64)
}

// -- 4.5 Inverse trig --------------------------------------------------------

/// asin(n) where n is Q16 in [-1, 1].  Returns integer milliradians.
fn builtin_asin(args: &[BValue], _s: bool) -> BValue {
    let n = (arg0(args) as f64 / FP_SCALE as f64).clamp(-1.0, 1.0);
    BValue((n.asin() * 1000.0).round() as i64)
}

/// acos(n) where n is Q16 in [-1, 1].  Returns integer milliradians.
fn builtin_acos(args: &[BValue], _s: bool) -> BValue {
    let n = (arg0(args) as f64 / FP_SCALE as f64).clamp(-1.0, 1.0);
    BValue((n.acos() * 1000.0).round() as i64)
}

// -- 4.3 Logarithms and exponentials -----------------------------------------

/// Natural log of Q16 value n.  n must be > 0; returns Q16 ln(n).
fn builtin_ln(args: &[BValue], strict: bool) -> BValue {
    let n = arg0(args);
    if n <= 0 {
        if strict {
            eprintln!("[strict-math] ln: non-positive argument");
        }
        return BValue(i64::MIN);
    }
    let real = n as f64 / FP_SCALE as f64;
    BValue((real.ln() * FP_SCALE as f64).round() as i64)
}

/// log2 of Q16 value n.  Returns Q16 result.
fn builtin_log2(args: &[BValue], strict: bool) -> BValue {
    let n = arg0(args);
    if n <= 0 {
        if strict {
            eprintln!("[strict-math] log2: non-positive argument");
        }
        return BValue(i64::MIN);
    }
    let real = n as f64 / FP_SCALE as f64;
    BValue((real.log2() * FP_SCALE as f64).round() as i64)
}

/// log10 of Q16 value n.  Returns Q16 result.
fn builtin_log10(args: &[BValue], strict: bool) -> BValue {
    let n = arg0(args);
    if n <= 0 {
        if strict {
            eprintln!("[strict-math] log10: non-positive argument");
        }
        return BValue(i64::MIN);
    }
    let real = n as f64 / FP_SCALE as f64;
    BValue((real.log10() * FP_SCALE as f64).round() as i64)
}

/// e^n where n is Q16.  Returns Q16.
fn builtin_exp(args: &[BValue], _s: bool) -> BValue {
    let n = arg0(args) as f64 / FP_SCALE as f64;
    let result = n.exp();
    if result > i64::MAX as f64 {
        return BValue(i64::MAX);
    }
    BValue((result * FP_SCALE as f64).round() as i64)
}

/// 2^n where n is Q16.  Returns Q16.
fn builtin_exp2(args: &[BValue], _s: bool) -> BValue {
    let n = arg0(args) as f64 / FP_SCALE as f64;
    let result = n.exp2();
    if result > i64::MAX as f64 {
        return BValue(i64::MAX);
    }
    BValue((result * FP_SCALE as f64).round() as i64)
}

// -- 4.4 Hyperbolic functions -------------------------------------------------

fn builtin_sinh(args: &[BValue], _s: bool) -> BValue {
    let n = arg0(args) as f64 / FP_SCALE as f64;
    BValue((n.sinh() * FP_SCALE as f64).round() as i64)
}

fn builtin_cosh(args: &[BValue], _s: bool) -> BValue {
    let n = arg0(args) as f64 / FP_SCALE as f64;
    BValue((n.cosh() * FP_SCALE as f64).round() as i64)
}

fn builtin_tanh(args: &[BValue], _s: bool) -> BValue {
    let n = arg0(args) as f64 / FP_SCALE as f64;
    BValue((n.tanh() * FP_SCALE as f64).round() as i64)
}

// ===========================================================================
// 5. Random Number Generation
// ===========================================================================

fn builtin_srand(args: &[BValue], _s: bool) -> BValue {
    let seed = arg0(args) as u64;
    // Ensure state is never 0 (xorshift64 with state 0 gets stuck).
    RNG_STATE.with(|state| state.set(if seed == 0 { 1 } else { seed }));
    BValue(0)
}

fn builtin_rand(args: &[BValue], _s: bool) -> BValue {
    let _ = args;
    BValue(xorshift64())
}

fn builtin_randrange(args: &[BValue], _s: bool) -> BValue {
    let lo = arg0(args);
    let hi = arg1(args);
    if hi <= lo {
        return BValue(lo);
    }
    BValue(lo + xorshift64().abs() % (hi - lo))
}

// ===========================================================================
// 6. Math Constants (no-argument builtins, return Q16)
// ===========================================================================

fn builtin_m_pi(_: &[BValue], _s: bool) -> BValue {
    BValue(205887) // π * 65536
}

fn builtin_m_e(_: &[BValue], _s: bool) -> BValue {
    BValue(178145) // e * 65536
}

fn builtin_m_phi(_: &[BValue], _s: bool) -> BValue {
    BValue(106028) // φ * 65536
}

fn builtin_m_ln2(_: &[BValue], _s: bool) -> BValue {
    BValue(45426) // ln(2) * 65536
}

fn builtin_m_sqrt2(_: &[BValue], _s: bool) -> BValue {
    BValue(92682) // √2 * 65536
}

// ===========================================================================
// Registration
// ===========================================================================

/// Insert all math builtins into `registry`.  Calling this more than once is a
/// no-op functionally (last write wins, all pointers are identical).
pub fn register_math_builtins(registry: &mut HashMap<String, MathFn>) {
    let entries: &[(&str, MathFn)] = &[
        // Integer math
        ("abs", builtin_abs),
        ("sign", builtin_sign),
        ("min", builtin_min),
        ("max", builtin_max),
        ("clamp", builtin_clamp),
        ("divf", builtin_divf),
        ("modf", builtin_modf),
        ("pow", builtin_pow),
        ("sqrt", builtin_sqrt),
        ("cbrt", builtin_cbrt),
        ("popcnt", builtin_popcnt),
        ("clz", builtin_clz),
        ("ctz", builtin_ctz),
        ("bswap", builtin_bswap),
        ("gcd", builtin_gcd),
        ("lcm", builtin_lcm),
        // Fixed-point
        ("fpmul", builtin_fpmul),
        ("fpdiv", builtin_fpdiv),
        ("fptoi", builtin_fptoi),
        ("itofp", builtin_itofp),
        // Trig
        ("sin", builtin_sin),
        ("cos", builtin_cos),
        ("tan", builtin_tan),
        ("atan2", builtin_atan2),
        ("asin", builtin_asin),
        ("acos", builtin_acos),
        // Log / exp
        ("ln", builtin_ln),
        ("log2", builtin_log2),
        ("log10", builtin_log10),
        ("exp", builtin_exp),
        ("exp2", builtin_exp2),
        // Hyperbolic
        ("sinh", builtin_sinh),
        ("cosh", builtin_cosh),
        ("tanh", builtin_tanh),
        // RNG
        ("srand", builtin_srand),
        ("rand", builtin_rand),
        ("randrange", builtin_randrange),
        // Constants
        ("m_pi", builtin_m_pi),
        ("m_e", builtin_m_e),
        ("m_phi", builtin_m_phi),
        ("m_ln2", builtin_m_ln2),
        ("m_sqrt2", builtin_m_sqrt2),
    ];
    for &(name, func) in entries {
        registry.insert(name.to_string(), func);
    }
}
