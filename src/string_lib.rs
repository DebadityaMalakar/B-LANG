/// B-LANG Phase 5 — String Standard Library
///
/// All functions operate on B-strings: heap vectors of `BValue` where each
/// element holds one ASCII character code, terminated by `BValue(4)` (EOT,
/// `\*e`).
///
/// Naming convention:
/// - Functions that write into a caller-supplied `dst` return `dst`.
/// - Functions that allocate a new heap vector return the heap base index
///   (encode_heap address) — the caller owns it and should rlsvec when done.
///
/// The public entry-point is `register_string_builtins`, which populates a
/// `Library` struct with both prefixed (`string::strlen`) and bare (`strlen`)
/// names.
use crate::builtins::STRING_TERMINATOR;
use crate::memory::{encode_heap, BValue};

// ---------------------------------------------------------------------------
// Low-level helpers
// ---------------------------------------------------------------------------

/// Walk a B-string stored as a slice of `BValue` starting at `base` inside
/// `data`, return its character codes (not including the terminator).
fn read_bstr(data: &[BValue], base: usize) -> Vec<i64> {
    let mut out = Vec::new();
    let mut i = base;
    while i < data.len() && data[i].0 != STRING_TERMINATOR {
        out.push(data[i].0);
        i += 1;
    }
    out
}

/// Write `chars` into `data` starting at `base`, append terminator.
/// `data` is grown if necessary.
fn write_bstr(data: &mut Vec<BValue>, base: usize, chars: &[i64]) {
    let needed = base + chars.len() + 1;
    if data.len() < needed {
        data.resize(needed, BValue(0));
    }
    for (i, &ch) in chars.iter().enumerate() {
        data[base + i] = BValue(ch);
    }
    data[base + chars.len()] = BValue(STRING_TERMINATOR);
}

/// Allocate a new heap slot large enough for `chars` + terminator, write
/// content, return the heap base index (raw usize, not encoded).
fn alloc_bstr(data: &mut Vec<BValue>, chars: &[i64]) -> usize {
    let base = data.len();
    for &ch in chars {
        data.push(BValue(ch));
    }
    data.push(BValue(STRING_TERMINATOR));
    base
}

// ---------------------------------------------------------------------------
// The string builtins are closures over a shared heap pointer.  Because
// `MathFn = fn(&[BValue], bool) -> BValue` is a *function pointer* (no
// captures), string functions that need heap access use a different strategy:
// they receive the heap address as a BValue and decode it themselves.
//
// The interpreter calls string builtins via the same math_registry dispatch:
//   math_fn(args, strict_math)
// `args` contains B-addresses (encoded i64), so the functions must work
// through the interpreter's load/store interface — except we don't have
// `&mut Interpreter` here.
//
// Solution: string functions that touch heap data are registered as
// `StringBuiltinFn` (which takes the raw heap Vec) and called from a thin
// shim in eval.rs that pulls the heap out before dispatching.
// ---------------------------------------------------------------------------

/// String builtin signature: receives argument B-values and a mutable
/// reference to the raw heap data (and the global/local data slices for
/// reading string arguments).
pub type StringBuiltinFn = fn(
    args: &[BValue],
    heap: &mut Vec<BValue>,
    strict: bool,
) -> BValue;

// ---------------------------------------------------------------------------
// Helpers for argument decoding (heap addresses carry HEAP_TAG)
// ---------------------------------------------------------------------------

const HEAP_TAG: i64 = 1_i64 << 61;
const LOCAL_TAG: i64 = 1_i64 << 62;

fn decode_heap_idx(addr: i64) -> Option<usize> {
    if (addr & LOCAL_TAG) != 0 {
        return None; // local address — caller must handle
    }
    if (addr & HEAP_TAG) != 0 {
        Some((addr & !HEAP_TAG) as usize)
    } else {
        // global address — treat as 0 base for our purposes (shouldn't reach here)
        None
    }
}

fn arg_heap_idx(args: &[BValue], n: usize) -> usize {
    let addr = args.get(n).copied().unwrap_or(BValue(0)).0;
    decode_heap_idx(addr).unwrap_or(0)
}

fn arg_i64(args: &[BValue], n: usize) -> i64 {
    args.get(n).copied().unwrap_or(BValue(0)).0
}

fn arg_addr(args: &[BValue], n: usize) -> i64 {
    args.get(n).copied().unwrap_or(BValue(0)).0
}

// ===========================================================================
// 3.1 Inspection
// ===========================================================================

fn builtin_strlen(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let base = arg_heap_idx(args, 0);
    let chars = read_bstr(heap, base);
    BValue(chars.len() as i64)
}

fn builtin_strcmp(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let a = read_bstr(heap, arg_heap_idx(args, 0));
    let b = read_bstr(heap, arg_heap_idx(args, 1));
    BValue(a.cmp(&b) as i64)
}

fn builtin_strcmpi(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let a: Vec<i64> = read_bstr(heap, arg_heap_idx(args, 0))
        .into_iter()
        .map(|c| ascii_tolower(c))
        .collect();
    let b: Vec<i64> = read_bstr(heap, arg_heap_idx(args, 1))
        .into_iter()
        .map(|c| ascii_tolower(c))
        .collect();
    BValue(a.cmp(&b) as i64)
}

fn builtin_startswith(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let s = read_bstr(heap, arg_heap_idx(args, 0));
    let pre = read_bstr(heap, arg_heap_idx(args, 1));
    BValue(if s.starts_with(pre.as_slice()) { 1 } else { 0 })
}

fn builtin_endswith(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let s = read_bstr(heap, arg_heap_idx(args, 0));
    let suf = read_bstr(heap, arg_heap_idx(args, 1));
    BValue(if s.ends_with(suf.as_slice()) { 1 } else { 0 })
}

fn builtin_contains(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let s = read_bstr(heap, arg_heap_idx(args, 0));
    let needle = read_bstr(heap, arg_heap_idx(args, 1));
    BValue(if find_subslice(&s, &needle).is_some() { 1 } else { 0 })
}

fn builtin_indexof(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let s = read_bstr(heap, arg_heap_idx(args, 0));
    let needle = read_bstr(heap, arg_heap_idx(args, 1));
    BValue(find_subslice(&s, &needle).map(|i| i as i64).unwrap_or(-1))
}

fn builtin_count(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let s = read_bstr(heap, arg_heap_idx(args, 0));
    let needle = read_bstr(heap, arg_heap_idx(args, 1));
    if needle.is_empty() {
        return BValue(0);
    }
    let mut count = 0i64;
    let mut pos = 0;
    while pos + needle.len() <= s.len() {
        if s[pos..pos + needle.len()] == needle[..] {
            count += 1;
            pos += needle.len();
        } else {
            pos += 1;
        }
    }
    BValue(count)
}

// ===========================================================================
// 3.2 Case conversion
// ===========================================================================

fn ascii_toupper(c: i64) -> i64 {
    if (b'a' as i64..=b'z' as i64).contains(&c) {
        c - 32
    } else {
        c
    }
}
fn ascii_tolower(c: i64) -> i64 {
    if (b'A' as i64..=b'Z' as i64).contains(&c) {
        c + 32
    } else {
        c
    }
}
fn is_upper(c: i64) -> bool {
    (b'A' as i64..=b'Z' as i64).contains(&c)
}
fn is_lower(c: i64) -> bool {
    (b'a' as i64..=b'z' as i64).contains(&c)
}
fn is_word_delim(c: i64) -> bool {
    c == b' ' as i64 || c == b'_' as i64
}

fn builtin_toupper(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let src = read_bstr(heap, arg_heap_idx(args, 0));
    let dst_addr = arg_addr(args, 1);
    let dst_base = decode_heap_idx(dst_addr).unwrap_or(0);
    let upper: Vec<i64> = src.into_iter().map(ascii_toupper).collect();
    write_bstr(heap, dst_base, &upper);
    BValue(dst_addr)
}

fn builtin_tolower(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let src = read_bstr(heap, arg_heap_idx(args, 0));
    let dst_addr = arg_addr(args, 1);
    let dst_base = decode_heap_idx(dst_addr).unwrap_or(0);
    let lower: Vec<i64> = src.into_iter().map(ascii_tolower).collect();
    write_bstr(heap, dst_base, &lower);
    BValue(dst_addr)
}

fn builtin_tocamel(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let src = read_bstr(heap, arg_heap_idx(args, 0));
    let dst_addr = arg_addr(args, 1);
    let dst_base = decode_heap_idx(dst_addr).unwrap_or(0);

    let mut out = Vec::new();
    let mut capitalize_next = false;
    let mut first_word = true;
    for ch in &src {
        if is_word_delim(*ch) {
            capitalize_next = true;
            first_word = false;
        } else if capitalize_next && !first_word {
            out.push(ascii_toupper(*ch));
            capitalize_next = false;
        } else if first_word {
            out.push(ascii_tolower(*ch));
        } else {
            out.push(*ch);
        }
    }
    write_bstr(heap, dst_base, &out);
    BValue(dst_addr)
}

fn builtin_tosnake(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let src = read_bstr(heap, arg_heap_idx(args, 0));
    let dst_addr = arg_addr(args, 1);
    let dst_base = decode_heap_idx(dst_addr).unwrap_or(0);

    let mut out = Vec::new();
    for (i, &ch) in src.iter().enumerate() {
        if ch == b' ' as i64 {
            out.push(b'_' as i64);
        } else if is_upper(ch) && i > 0 && is_lower(src[i - 1]) {
            out.push(b'_' as i64);
            out.push(ascii_tolower(ch));
        } else {
            out.push(ascii_tolower(ch));
        }
    }
    write_bstr(heap, dst_base, &out);
    BValue(dst_addr)
}

fn builtin_totitle(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let src = read_bstr(heap, arg_heap_idx(args, 0));
    let dst_addr = arg_addr(args, 1);
    let dst_base = decode_heap_idx(dst_addr).unwrap_or(0);

    let mut out = Vec::new();
    let mut cap = true;
    for &ch in &src {
        if ch == b' ' as i64 {
            out.push(ch);
            cap = true;
        } else if cap {
            out.push(ascii_toupper(ch));
            cap = false;
        } else {
            out.push(ascii_tolower(ch));
        }
    }
    write_bstr(heap, dst_base, &out);
    BValue(dst_addr)
}

fn builtin_capitalize(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let src = read_bstr(heap, arg_heap_idx(args, 0));
    let dst_addr = arg_addr(args, 1);
    let dst_base = decode_heap_idx(dst_addr).unwrap_or(0);

    let mut out = src.clone();
    if let Some(first) = out.first_mut() {
        *first = ascii_toupper(*first);
    }
    write_bstr(heap, dst_base, &out);
    BValue(dst_addr)
}

// ===========================================================================
// 3.3 Search and replace
// ===========================================================================

fn replace_impl(src: &[i64], from: &[i64], to: &[i64], max: usize) -> Vec<i64> {
    if from.is_empty() {
        return src.to_vec();
    }
    let mut out = Vec::new();
    let mut pos = 0;
    let mut count = 0;
    while pos < src.len() {
        if count < max && pos + from.len() <= src.len() && src[pos..pos + from.len()] == *from {
            out.extend_from_slice(to);
            pos += from.len();
            count += 1;
        } else {
            out.push(src[pos]);
            pos += 1;
        }
    }
    out
}

fn builtin_replace(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let src = read_bstr(heap, arg_heap_idx(args, 0));
    let from = read_bstr(heap, arg_heap_idx(args, 1));
    let to = read_bstr(heap, arg_heap_idx(args, 2));
    let dst_addr = arg_addr(args, 3);
    let dst_base = decode_heap_idx(dst_addr).unwrap_or(0);
    let result = replace_impl(&src, &from, &to, usize::MAX);
    write_bstr(heap, dst_base, &result);
    BValue(dst_addr)
}

fn builtin_replacen(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let src = read_bstr(heap, arg_heap_idx(args, 0));
    let from = read_bstr(heap, arg_heap_idx(args, 1));
    let to = read_bstr(heap, arg_heap_idx(args, 2));
    let n = arg_i64(args, 3).max(0) as usize;
    let dst_addr = arg_addr(args, 4);
    let dst_base = decode_heap_idx(dst_addr).unwrap_or(0);
    let result = replace_impl(&src, &from, &to, n);
    write_bstr(heap, dst_base, &result);
    BValue(dst_addr)
}

// ===========================================================================
// 3.4 Trimming
// ===========================================================================

fn is_ws(c: i64) -> bool {
    matches!(c, 9 | 10 | 13 | 32) // \t \n \r space
}

fn builtin_strip(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let src = read_bstr(heap, arg_heap_idx(args, 0));
    let dst_addr = arg_addr(args, 1);
    let dst_base = decode_heap_idx(dst_addr).unwrap_or(0);
    let trimmed: Vec<i64> = src
        .iter()
        .copied()
        .skip_while(|&c| is_ws(c))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .skip_while(|&c| is_ws(c))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    write_bstr(heap, dst_base, &trimmed);
    BValue(dst_addr)
}

fn builtin_lstrip(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let src = read_bstr(heap, arg_heap_idx(args, 0));
    let dst_addr = arg_addr(args, 1);
    let dst_base = decode_heap_idx(dst_addr).unwrap_or(0);
    let trimmed: Vec<i64> = src.into_iter().skip_while(|&c| is_ws(c)).collect();
    write_bstr(heap, dst_base, &trimmed);
    BValue(dst_addr)
}

fn builtin_rstrip(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let src = read_bstr(heap, arg_heap_idx(args, 0));
    let dst_addr = arg_addr(args, 1);
    let dst_base = decode_heap_idx(dst_addr).unwrap_or(0);
    let mut trimmed = src;
    while trimmed.last().copied().map(is_ws).unwrap_or(false) {
        trimmed.pop();
    }
    write_bstr(heap, dst_base, &trimmed);
    BValue(dst_addr)
}

// ===========================================================================
// 3.5 Padding  (return encoded heap address of new allocation)
// ===========================================================================

fn builtin_lpad(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let src = read_bstr(heap, arg_heap_idx(args, 0));
    let width = arg_i64(args, 1).max(0) as usize;
    let ch = arg_i64(args, 2);
    let result = if src.len() >= width {
        src
    } else {
        let pad = width - src.len();
        let mut out = vec![ch; pad];
        out.extend_from_slice(&src);
        out
    };
    let base = alloc_bstr(heap, &result);
    BValue(encode_heap(base))
}

fn builtin_rpad(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let src = read_bstr(heap, arg_heap_idx(args, 0));
    let width = arg_i64(args, 1).max(0) as usize;
    let ch = arg_i64(args, 2);
    let result = if src.len() >= width {
        src
    } else {
        let mut out = src;
        while out.len() < width {
            out.push(ch);
        }
        out
    };
    let base = alloc_bstr(heap, &result);
    BValue(encode_heap(base))
}

fn builtin_pad(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let src = read_bstr(heap, arg_heap_idx(args, 0));
    let width = arg_i64(args, 1).max(0) as usize;
    let ch = arg_i64(args, 2);
    let result = if src.len() >= width {
        src.clone()
    } else {
        let total_pad = width - src.len();
        let left_pad = total_pad / 2;
        let right_pad = total_pad - left_pad;
        let mut out = vec![ch; left_pad];
        out.extend_from_slice(&src);
        for _ in 0..right_pad {
            out.push(ch);
        }
        out
    };
    let base = alloc_bstr(heap, &result);
    BValue(encode_heap(base))
}

// ===========================================================================
// 3.6 Repetition
// ===========================================================================

fn builtin_repeat(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let src = read_bstr(heap, arg_heap_idx(args, 0));
    let n = arg_i64(args, 1).max(0) as usize;
    let mut result = Vec::with_capacity(src.len() * n);
    for _ in 0..n {
        result.extend_from_slice(&src);
    }
    let base = alloc_bstr(heap, &result);
    BValue(encode_heap(base))
}

// ===========================================================================
// 3.7 Substring
// ===========================================================================

fn builtin_substr(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let src = read_bstr(heap, arg_heap_idx(args, 0));
    let start = (arg_i64(args, 1).max(0) as usize).min(src.len());
    let len = (arg_i64(args, 2).max(0) as usize).min(src.len() - start);
    let dst_addr = arg_addr(args, 3);
    let dst_base = decode_heap_idx(dst_addr).unwrap_or(0);
    let slice = &src[start..start + len];
    write_bstr(heap, dst_base, slice);
    BValue(dst_addr)
}

fn builtin_slice(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let src = read_bstr(heap, arg_heap_idx(args, 0));
    let start = (arg_i64(args, 1).max(0) as usize).min(src.len());
    let end = (arg_i64(args, 2).max(0) as usize).min(src.len());
    let dst_addr = arg_addr(args, 3);
    let dst_base = decode_heap_idx(dst_addr).unwrap_or(0);
    let slice = if start <= end { &src[start..end] } else { &[] as &[i64] };
    write_bstr(heap, dst_base, slice);
    BValue(dst_addr)
}

// ===========================================================================
// 3.8 Number conversion
// ===========================================================================

fn builtin_itoa(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let n = arg_i64(args, 0);
    let dst_addr = arg_addr(args, 1);
    let dst_base = decode_heap_idx(dst_addr).unwrap_or(0);
    let s = n.to_string();
    let chars: Vec<i64> = s.bytes().map(|b| b as i64).collect();
    write_bstr(heap, dst_base, &chars);
    BValue(dst_addr)
}

fn builtin_itoao(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let n = arg_i64(args, 0);
    let dst_addr = arg_addr(args, 1);
    let dst_base = decode_heap_idx(dst_addr).unwrap_or(0);
    let s = format!("{:o}", n);
    let chars: Vec<i64> = s.bytes().map(|b| b as i64).collect();
    write_bstr(heap, dst_base, &chars);
    BValue(dst_addr)
}

fn builtin_itoax(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let n = arg_i64(args, 0);
    let dst_addr = arg_addr(args, 1);
    let dst_base = decode_heap_idx(dst_addr).unwrap_or(0);
    let s = format!("{:x}", n);
    let chars: Vec<i64> = s.bytes().map(|b| b as i64).collect();
    write_bstr(heap, dst_base, &chars);
    BValue(dst_addr)
}

fn builtin_atoi(args: &[BValue], heap: &mut Vec<BValue>, _s: bool) -> BValue {
    let chars = read_bstr(heap, arg_heap_idx(args, 0));
    // skip leading whitespace
    let trimmed: Vec<u8> = chars
        .iter()
        .skip_while(|&&c| is_ws(c))
        .map(|&c| (c & 0xFF) as u8)
        .collect();
    let s = String::from_utf8_lossy(&trimmed);
    // parse as many leading digits (optionally preceded by '-') as possible
    let mut end = 0;
    let bytes = s.as_bytes();
    if end < bytes.len() && bytes[end] == b'-' {
        end += 1;
    }
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    let result = s[..end].parse::<i64>().unwrap_or(0);
    BValue(result)
}

// ===========================================================================
// 3.9 Character classification
// ===========================================================================

fn builtin_isalpha(args: &[BValue], _h: &mut Vec<BValue>, _s: bool) -> BValue {
    let c = args.first().copied().unwrap_or(BValue(0)).0;
    BValue(if (b'a' as i64..=b'z' as i64).contains(&c) || (b'A' as i64..=b'Z' as i64).contains(&c) { 1 } else { 0 })
}

fn builtin_isdigit(args: &[BValue], _h: &mut Vec<BValue>, _s: bool) -> BValue {
    let c = args.first().copied().unwrap_or(BValue(0)).0;
    BValue(if (b'0' as i64..=b'9' as i64).contains(&c) { 1 } else { 0 })
}

fn builtin_isalnum(args: &[BValue], _h: &mut Vec<BValue>, _s: bool) -> BValue {
    let c = args.first().copied().unwrap_or(BValue(0)).0;
    BValue(if (b'a' as i64..=b'z' as i64).contains(&c)
        || (b'A' as i64..=b'Z' as i64).contains(&c)
        || (b'0' as i64..=b'9' as i64).contains(&c) { 1 } else { 0 })
}

fn builtin_isspace(args: &[BValue], _h: &mut Vec<BValue>, _s: bool) -> BValue {
    let c = args.first().copied().unwrap_or(BValue(0)).0;
    BValue(if is_ws(c) { 1 } else { 0 })
}

fn builtin_isupper(args: &[BValue], _h: &mut Vec<BValue>, _s: bool) -> BValue {
    let c = args.first().copied().unwrap_or(BValue(0)).0;
    BValue(if is_upper(c) { 1 } else { 0 })
}

fn builtin_islower(args: &[BValue], _h: &mut Vec<BValue>, _s: bool) -> BValue {
    let c = args.first().copied().unwrap_or(BValue(0)).0;
    BValue(if is_lower(c) { 1 } else { 0 })
}

// ===========================================================================
// Internal utility
// ===========================================================================

fn find_subslice(haystack: &[i64], needle: &[i64]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
}

// ===========================================================================
// Registration
// ===========================================================================

/// All string builtins as `(name, StringBuiltinFn)` pairs.
pub const STRING_BUILTINS: &[(&str, StringBuiltinFn)] = &[
    // Inspection
    ("strlen", builtin_strlen),
    ("strcmp", builtin_strcmp),
    ("strcmpi", builtin_strcmpi),
    ("startswith", builtin_startswith),
    ("endswith", builtin_endswith),
    ("contains", builtin_contains),
    ("indexof", builtin_indexof),
    ("count", builtin_count),
    // Case
    ("toupper", builtin_toupper),
    ("tolower", builtin_tolower),
    ("tocamel", builtin_tocamel),
    ("tosnake", builtin_tosnake),
    ("totitle", builtin_totitle),
    ("capitalize", builtin_capitalize),
    // Search / replace
    ("replace", builtin_replace),
    ("replacen", builtin_replacen),
    // Trim
    ("strip", builtin_strip),
    ("lstrip", builtin_lstrip),
    ("rstrip", builtin_rstrip),
    // Pad (return heap address)
    ("lpad", builtin_lpad),
    ("rpad", builtin_rpad),
    ("pad", builtin_pad),
    // Repeat
    ("repeat", builtin_repeat),
    // Substring
    ("substr", builtin_substr),
    ("slice", builtin_slice),
    // Number conversion
    ("itoa", builtin_itoa),
    ("itoao", builtin_itoao),
    ("itoax", builtin_itoax),
    ("atoi", builtin_atoi),
    // Classification
    ("isalpha", builtin_isalpha),
    ("isdigit", builtin_isdigit),
    ("isalnum", builtin_isalnum),
    ("isspace", builtin_isspace),
    ("isupper", builtin_isupper),
    ("islower", builtin_islower),
];

