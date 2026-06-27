//! LaTeX arithmetic preprocessor.
//!
//! Rewrites a practical subset of LaTeX math into the ASCII/unicode forms the
//! tokenizer already understands, plus a `root{index,radicand}` sentinel for
//! roots (see [`crate::tokenizer::Token::Root`]).
//!
//! Supported: `\frac{a}{b}`, `\dfrac`, `\tfrac`, `\sqrt[n]{a}`, `\sqrt{a}`,
//! `\times`, `\cdot`, `\div`, `\pi`, `\$`, `\%`, `\{`, `\}`, thin/hair spacing
//! (`\, \; \: \!`), `\left( \right)`, `\left[ \right]`.
//!
//! Everything else is passed through unchanged so plain NL input is unaffected.

const PI: &str = "3.141592653589793";

/// Expand LaTeX commands in `input`. O(n) single pass, allocating only when a
/// LaTeX command (`\`) is actually present.
pub fn expand(input: &str) -> String {
    if !input.contains('\\') {
        return input.to_string();
    }
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b != b'\\' {
            out.push(b as char);
            i += 1;
            continue;
        }
        if i + 1 >= bytes.len() {
            out.push('\\');
            break;
        }
        let next = bytes[i + 1];
        if !next.is_ascii_alphabetic() {
            out.push(escape_char(next));
            i += 2;
            continue;
        }
        let mut j = i + 1;
        while j < bytes.len() && bytes[j].is_ascii_alphabetic() {
            j += 1;
        }
        let name = &input[i + 1..j];
        i = expand_command(name, input, j, &mut out);
    }
    out
}

fn escape_char(b: u8) -> char {
    match b {
        b'$' | b'%' | b'{' | b'}' | b'#' | b'&' | b'_' | b'\\' => b as char,
        other => other as char,
    }
}

fn expand_command(name: &str, src: &str, j: usize, out: &mut String) -> usize {
    let mut single = |c: char, next: usize| {
        out.push(c);
        next
    };
    match name {
        "times" | "cdot" => single('×', j),
        "div" => single('÷', j),
        "pi" => {
            out.push_str(PI);
            j
        }
        "left" | "right" => j, // drop the qualifier; the bracket char follows
        "," | ";" | ":" | "!" | "quad" | "qquad" => single(' ', j),
        "frac" | "dfrac" | "tfrac" => {
            let (a, k) = read_group(src, j);
            let (b, k) = read_group(src, k);
            out.push_str("( ");
            out.push_str(&expand(&a));
            out.push_str(" ) / ( ");
            out.push_str(&expand(&b));
            out.push_str(" )");
            k
        }
        "sqrt" => {
            let (index, k0) = read_optional_index(src, j);
            let (rad, k) = read_group(src, k0);
            let idx = index.unwrap_or(2.0);
            out.push_str("root{");
            out.push_str(&fmt_index(idx));
            out.push(',');
            out.push_str(&expand(&rad));
            out.push('}');
            k
        }
        _ => {
            // unknown command: pass through verbatim including the backslash
            out.push('\\');
            out.push_str(name);
            j
        }
    }
}

fn fmt_index(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Read `{ ... }` (with nested braces). Returns inner content (trimmed) and the
/// index just past the closing brace. If there is no group, returns ("", at).
fn read_group(src: &str, mut at: usize) -> (String, usize) {
    let bytes = src.as_bytes();
    while at < bytes.len() && bytes[at].is_ascii_whitespace() {
        at += 1;
    }
    if at >= bytes.len() || bytes[at] != b'{' {
        return (String::new(), at);
    }
    let start = at + 1;
    let mut depth = 1;
    let mut k = start;
    while k < bytes.len() {
        match bytes[k] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return (src[start..k].trim().to_string(), k + 1);
                }
            }
            _ => {}
        }
        k += 1;
    }
    (src[start..].trim().to_string(), k)
}

/// Read an optional `[index]` after `\sqrt`. Returns (index, index past `]`).
fn read_optional_index(src: &str, mut at: usize) -> (Option<f64>, usize) {
    let bytes = src.as_bytes();
    while at < bytes.len() && bytes[at].is_ascii_whitespace() {
        at += 1;
    }
    if at >= bytes.len() || bytes[at] != b'[' {
        return (None, at);
    }
    let start = at + 1;
    let mut k = start;
    while k < bytes.len() && bytes[k] != b']' {
        k += 1;
    }
    let inner = src[start..k].trim();
    let idx = inner.parse::<f64>().ok();
    let next = if k < bytes.len() { k + 1 } else { k };
    (idx, next)
}

#[cfg(test)]
mod tests {
    use super::expand;

    #[test]
    fn passthrough_when_no_latex() {
        assert_eq!(expand("I have 2 dogs"), "I have 2 dogs");
    }

    #[test]
    fn frac_becomes_division() {
        assert_eq!(expand("\\frac{6}{2}"), "( 6 ) / ( 2 )");
    }

    #[test]
    fn frac_nested_args() {
        assert_eq!(expand("\\frac{2 + 3}{4}"), "( 2 + 3 ) / ( 4 )");
    }

    #[test]
    fn times_cdot_div() {
        assert_eq!(expand("3 \\times 4"), "3 × 4");
        assert_eq!(expand("3 \\cdot 4"), "3 × 4");
        assert_eq!(expand("12 \\div 3"), "12 ÷ 3");
    }

    #[test]
    fn sqrt_plain() {
        assert_eq!(expand("\\sqrt{9}"), "root{2,9}");
    }

    #[test]
    fn sqrt_indexed() {
        assert_eq!(expand("\\sqrt[3]{27}"), "root{3,27}");
    }

    #[test]
    fn pi_and_escapes() {
        assert_eq!(expand("\\pi"), super::PI);
        assert_eq!(expand("10\\$"), "10$");
        assert_eq!(expand("20\\%"), "20%");
    }

    #[test]
    fn left_right_dropped() {
        assert_eq!(expand("\\left( 1 \\right)"), "( 1 )");
    }

    #[test]
    fn unknown_command_passthrough() {
        assert_eq!(expand("5 \\alpha 3"), "5 \\alpha 3");
    }
}
