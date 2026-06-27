//! Expression evaluator — pure arithmetic, recursive descent.
//!
//! Handles operator precedence (`* /` bind tighter than `+ -`), parentheses,
//! unary `+`/`-`, and a fixed set of functions/constants. Self-gating: the
//! lexer accepts ONLY numbers, arithmetic operators, parens, and the known
//! identifier set, so natural-language inputs fall through to the NL
//! classifier unchanged. No dependencies, zero-allocation lexing.

#[derive(Debug, Clone, Copy, PartialEq)]
enum FuncKind {
    Sin,
    Cos,
    Tan,
    Sqrt,
    Log,
    Ln,
    Exp,
    Abs,
    Floor,
    Ceil,
    Asin,
    Acos,
    Atan,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tok {
    Num(f64),
    Const(f64),
    Func(FuncKind),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
}

/// Lex `s` into arithmetic tokens. Returns `None` if any char is not a digit,
/// decimal point, operator, parenthesis, or whitespace.
fn lex(s: &str) -> Option<Vec<Tok>> {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b' ' | b'\t' | b'\n' | b'\r' => i += 1,
            b'+' => {
                out.push(Tok::Plus);
                i += 1;
            }
            b'-' => {
                out.push(Tok::Minus);
                i += 1;
            }
            b'*' => {
                out.push(Tok::Star);
                i += 1;
            }
            b'/' => {
                out.push(Tok::Slash);
                i += 1;
            }
            b'^' => {
                out.push(Tok::Caret);
                i += 1;
            }
            b'(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            b')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            b'0'..=b'9' | b'.' => {
                let start = i;
                let mut saw_dot = false;
                while i < bytes.len() {
                    match bytes[i] {
                        b'0'..=b'9' => i += 1,
                        b'.' if !saw_dot => {
                            saw_dot = true;
                            i += 1;
                        }
                        _ => break,
                    }
                }
                let v: f64 = s[start..i].parse().ok()?;
                if !v.is_finite() {
                    return None;
                }
                out.push(Tok::Num(v));
            }
            b'a'..=b'z' | b'A'..=b'Z' => {
                // Strict identifier gate: only the known function/constant
                // names are accepted. Any other letter run rejects the whole
                // input, routing it to the NL classifier.
                let start = i;
                while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let name = &s[start..i];
                match name.to_ascii_lowercase().as_str() {
                    "pi" => out.push(Tok::Const(std::f64::consts::PI)),
                    "e" => out.push(Tok::Const(std::f64::consts::E)),
                    "sin" => out.push(Tok::Func(FuncKind::Sin)),
                    "cos" => out.push(Tok::Func(FuncKind::Cos)),
                    "tan" => out.push(Tok::Func(FuncKind::Tan)),
                    "sqrt" => out.push(Tok::Func(FuncKind::Sqrt)),
                    "log" => out.push(Tok::Func(FuncKind::Log)),
                    "ln" => out.push(Tok::Func(FuncKind::Ln)),
                    "exp" => out.push(Tok::Func(FuncKind::Exp)),
                    "abs" => out.push(Tok::Func(FuncKind::Abs)),
                    "floor" => out.push(Tok::Func(FuncKind::Floor)),
                    "ceil" => out.push(Tok::Func(FuncKind::Ceil)),
                    "asin" => out.push(Tok::Func(FuncKind::Asin)),
                    "acos" => out.push(Tok::Func(FuncKind::Acos)),
                    "atan" => out.push(Tok::Func(FuncKind::Atan)),
                    _ => return None,
                }
            }
            _ if s[i..].starts_with('×') => {
                out.push(Tok::Star);
                i += '×'.len_utf8();
            }
            _ if s[i..].starts_with('÷') => {
                out.push(Tok::Slash);
                i += '÷'.len_utf8();
            }
            _ => return None,
        }
    }
    Some(out)
}

struct Parser<'a> {
    toks: &'a [Tok],
    pos: usize,
    bin_ops: usize,
    only_op: Option<Tok>,
    has_func: bool,
    has_const: bool,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<Tok> {
        self.toks.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.peek()?;
        self.pos += 1;
        Some(t)
    }

    fn note_binop(&mut self, t: Tok) {
        self.bin_ops += 1;
        // Track whether every binary op is the same kind (for single-op labels).
        self.only_op = match self.only_op {
            None => Some(t),
            Some(prev) if prev == t => Some(t),
            Some(_) => None,
        };
    }

    // expr := term (('+' | '-') term)*
    fn expr(&mut self) -> Option<f64> {
        let mut acc = self.term()?;
        while let Some(t) = self.peek() {
            match t {
                Tok::Plus => {
                    self.next();
                    self.note_binop(Tok::Plus);
                    acc += self.term()?;
                }
                Tok::Minus => {
                    self.next();
                    self.note_binop(Tok::Minus);
                    acc -= self.term()?;
                }
                _ => break,
            }
        }
        Some(acc)
    }

    // term := unary (('*' | '/') unary | <implicit> unary)*
    // Implicit multiplication: two primaries juxtaposed (`2(3)`, `2 pi`,
    // `(2)(3)`) multiply. The right operand is `unary`, so `2(-3)` works but
    // `2 - 3` stays subtraction (the `-` is consumed at `expr`).
    fn term(&mut self) -> Option<f64> {
        let mut acc = self.unary()?;
        loop {
            match self.peek() {
                Some(Tok::Star) => {
                    self.next();
                    self.note_binop(Tok::Star);
                    acc *= self.unary()?;
                }
                Some(Tok::Slash) => {
                    self.next();
                    self.note_binop(Tok::Slash);
                    let d = self.unary()?;
                    if d == 0.0 {
                        return None;
                    }
                    acc /= d;
                }
                Some(t) if is_implicit_starter(t) => {
                    self.note_binop(Tok::Star);
                    acc *= self.unary()?;
                }
                _ => break,
            }
        }
        Some(acc)
    }

    // unary := ('+' | '-') unary | power
    fn unary(&mut self) -> Option<f64> {
        match self.peek()? {
            Tok::Plus => {
                self.next();
                self.unary()
            }
            Tok::Minus => {
                self.next();
                Some(-self.unary()?)
            }
            _ => self.power(),
        }
    }

    // power := primary ('^' unary)?   (right-associative; exponent may be signed)
    fn power(&mut self) -> Option<f64> {
        let base = self.primary()?;
        if matches!(self.peek(), Some(Tok::Caret)) {
            self.next();
            self.note_binop(Tok::Caret);
            let exp = self.unary()?;
            apply_pow(base, exp)
        } else {
            Some(base)
        }
    }

    // primary := number | constant | func '(' expr ')' | '(' expr ')'
    fn primary(&mut self) -> Option<f64> {
        match self.next()? {
            Tok::Num(n) => Some(n),
            Tok::Const(c) => {
                self.has_const = true;
                Some(c)
            }
            Tok::Func(f) => {
                self.has_func = true;
                // Function calls require parentheses around their argument.
                (self.next()? == Tok::LParen).then_some(())?;
                let v = self.expr()?;
                (self.next()? == Tok::RParen).then_some(())?;
                apply_func(f, v)
            }
            Tok::LParen => {
                let v = self.expr()?;
                (self.next()? == Tok::RParen).then_some(v)
            }
            _ => None,
        }
    }
}

/// `pow` with a guard: reject non-finite results and 0^negative (division by zero).
fn apply_pow(base: f64, exp: f64) -> Option<f64> {
    if base == 0.0 && exp < 0.0 {
        return None;
    }
    let r = base.powf(exp);
    r.is_finite().then_some(r)
}

/// Tokens that trigger implicit multiplication when they follow a value at
/// the term level. A bare number literal is deliberately excluded: `2 3` is
/// ambiguous (probably a mistake), while `2 pi`, `2(3)`, `(2)(3)`, `2 sin(0)`
/// are clearly intended multiplication.
fn is_implicit_starter(t: Tok) -> bool {
    matches!(t, Tok::Const(_) | Tok::Func(_) | Tok::LParen)
}

/// Apply a named function. Returns `None` outside the function's domain
/// (e.g. `sqrt(-1)`, `ln(0)`), so the whole expression is rejected.
fn apply_func(f: FuncKind, v: f64) -> Option<f64> {
    let r = match f {
        FuncKind::Sin => v.sin(),
        FuncKind::Cos => v.cos(),
        FuncKind::Tan => v.tan(),
        FuncKind::Sqrt if v >= 0.0 => v.sqrt(),
        FuncKind::Log if v > 0.0 => v.log10(),
        FuncKind::Ln if v > 0.0 => v.ln(),
        FuncKind::Exp => v.exp(),
        FuncKind::Abs => v.abs(),
        FuncKind::Floor => v.floor(),
        FuncKind::Ceil => v.ceil(),
        FuncKind::Asin if (-1.0..=1.0).contains(&v) => v.asin(),
        FuncKind::Acos if (-1.0..=1.0).contains(&v) => v.acos(),
        FuncKind::Atan => v.atan(),
        _ => return None,
    };
    r.is_finite().then_some(r)
}

/// Decide the answer label. A lone binary op (with no function) keeps its
/// specific name; everything compound, or any function/constant, is `result`.
fn label_of(bin_ops: usize, only_op: Option<Tok>, has_func: bool) -> &'static str {
    if !has_func && bin_ops == 1 {
        match only_op {
            Some(Tok::Plus) => "sum",
            Some(Tok::Minus) => "difference",
            Some(Tok::Star) => "product",
            Some(Tok::Slash) => "quotient",
            _ => "result",
        }
    } else {
        "result"
    }
}

fn run_parser(s: &str) -> Option<ExprSummary> {
    let toks = lex(s)?;
    let mut p = Parser {
        toks: &toks,
        pos: 0,
        bin_ops: 0,
        only_op: None,
        has_func: false,
        has_const: false,
    };
    let v = p.expr()?;
    // Every token must be consumed, otherwise it was not a single expression.
    (p.pos == toks.len()).then_some(ExprSummary {
        value: v,
        bin_ops: p.bin_ops,
        only_op: p.only_op,
        has_func: p.has_func,
        has_const: p.has_const,
    })
}

struct ExprSummary {
    value: f64,
    bin_ops: usize,
    only_op: Option<Tok>,
    has_func: bool,
    has_const: bool,
}

/// True if `name` is a recognized function or constant (case-insensitive).
/// The same set the lexer accepts. Used by the transformer to keep a limit
/// variable from colliding with a known identifier (e.g. `lim sin -> 0 of sin`).
pub fn is_known_ident(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "pi" | "e"
            | "sin"
            | "cos"
            | "tan"
            | "sqrt"
            | "log"
            | "ln"
            | "exp"
            | "abs"
            | "floor"
            | "ceil"
            | "asin"
            | "acos"
            | "atan"
    )
}

/// Evaluate a pure arithmetic expression. Returns `None` for anything that is
/// not a complete arithmetic expression (unknown chars, trailing ops, etc.).
pub fn evaluate(s: &str) -> Option<f64> {
    run_parser(s).map(|e| e.value)
}

/// Evaluate and pick a label. Returns `None` unless the input is a complete
/// arithmetic expression with at least one binary operator, function call, or
/// constant — so a bare number literal falls through to the NL `Single` path.
pub fn evaluate_labelled(s: &str) -> Option<(f64, &'static str)> {
    let e = run_parser(s)?;
    if e.bin_ops == 0 && !e.has_func && !e.has_const {
        return None;
    }
    Some((e.value, label_of(e.bin_ops, e.only_op, e.has_func)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(s: &str) -> Option<f64> {
        evaluate(s)
    }

    #[test]
    fn precedence_mul_before_add() {
        assert_eq!(ev("3 + 4 * 2"), Some(11.0));
        assert_eq!(ev("2 * 3 + 4"), Some(10.0));
    }

    #[test]
    fn precedence_div_before_sub() {
        assert_eq!(ev("10 - 6 / 2"), Some(7.0));
    }

    #[test]
    fn parentheses_override() {
        assert_eq!(ev("(3 + 4) * 2"), Some(14.0));
        assert_eq!(ev("(1 + 2) * (3 + 4)"), Some(21.0));
        assert_eq!(ev("2 * (3 + 4)"), Some(14.0));
    }

    #[test]
    fn nested_parens() {
        assert_eq!(ev("((1 + 2) * 3) + 4"), Some(13.0));
    }

    #[test]
    fn unary_minus() {
        assert_eq!(ev("-3 + 2"), Some(-1.0));
        assert_eq!(ev("-(2 + 3)"), Some(-5.0));
        assert_eq!(ev("--2"), Some(2.0));
    }

    #[test]
    fn left_associative_division() {
        assert_eq!(ev("100 / 4 / 5"), Some(5.0));
        assert_eq!(ev("10 / 2 / 5"), Some(1.0));
    }

    #[test]
    fn decimals() {
        assert_eq!(ev("0.5 + 0.25"), Some(0.75));
        assert_eq!(ev("1.5 * 2"), Some(3.0));
    }

    #[test]
    fn implicit_multiplication() {
        assert_eq!(ev("2(3)"), Some(6.0));
        assert!((ev("2 pi").unwrap() - 2.0 * std::f64::consts::PI).abs() < 1e-12);
        assert_eq!(ev("(2)(3)"), Some(6.0));
        assert_eq!(ev("2(3 + 1)"), Some(8.0));
        assert_eq!(ev("3(2)(4)"), Some(24.0));
    }

    #[test]
    fn implicit_mult_vs_subtraction() {
        // `2 - 3` is subtraction, not `2 * (-3)`.
        assert_eq!(ev("2 - 3"), Some(-1.0));
        assert_eq!(ev("2(-3)"), Some(-6.0));
    }

    #[test]
    fn implicit_mult_counts_as_product_label() {
        assert_eq!(evaluate_labelled("2(3)").unwrap(), (6.0, "product"));
        assert_eq!(evaluate_labelled("2 pi").unwrap().1, "product");
    }

    #[test]
    fn implicit_mult_with_function() {
        assert_eq!(ev("2 sin(0)"), Some(0.0));
        assert!((ev("2 sqrt(2)").unwrap() - 2.0 * 2.0_f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn more_functions() {
        assert_eq!(ev("abs(-5)"), Some(5.0));
        assert_eq!(ev("abs(5)"), Some(5.0));
        assert_eq!(ev("floor(2.9)"), Some(2.0));
        assert_eq!(ev("ceil(2.1)"), Some(3.0));
        assert!((ev("asin(1)").unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!((ev("acos(0)").unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!((ev("atan(1)").unwrap() - std::f64::consts::FRAC_PI_4).abs() < 1e-12);
    }

    #[test]
    fn inverse_trig_domain() {
        // asin/acos undefined outside [-1, 1].
        assert_eq!(ev("asin(2)"), None);
        assert_eq!(ev("acos(-2)"), None);
    }

    #[test]
    fn abs_in_expression() {
        assert_eq!(ev("abs(-3) + 1"), Some(4.0));
        assert_eq!(ev("2 * abs(-4)"), Some(8.0));
    }

    #[test]
    fn power_basic() {
        assert_eq!(ev("2 ^ 3"), Some(8.0));
        assert_eq!(ev("2 ^ 10"), Some(1024.0));
        assert!((ev("9 ^ 0.5").unwrap() - 3.0).abs() < 1e-12);
    }

    #[test]
    fn power_right_associative() {
        // 2^3^2 = 2^(3^2) = 2^9 = 512, not (2^3)^2 = 64.
        assert_eq!(ev("2 ^ 3 ^ 2"), Some(512.0));
    }

    #[test]
    fn power_binds_tighter_than_unary_minus() {
        // Standard math convention: -2^2 = -(2^2) = -4.
        assert_eq!(ev("-2 ^ 2"), Some(-4.0));
    }

    #[test]
    fn power_with_signed_exponent() {
        assert_eq!(ev("2 ^ -1"), Some(0.5));
        assert_eq!(ev("2 ^ +3"), Some(8.0));
    }

    #[test]
    fn power_precedence_vs_mul() {
        // ^ binds tighter than *: 2 * 3 ^ 2 = 2 * 9 = 18.
        assert_eq!(ev("2 * 3 ^ 2"), Some(18.0));
        assert_eq!(ev("(2 * 3) ^ 2"), Some(36.0));
    }

    #[test]
    fn power_domain_errors() {
        assert_eq!(ev("0 ^ -1"), None); // 1/0
        assert_eq!(ev("0 ^ 0"), Some(1.0));
    }

    #[test]
    fn unicode_operators() {
        assert_eq!(ev("3 × 4"), Some(12.0));
        assert_eq!(ev("12 ÷ 3"), Some(4.0));
    }

    #[test]
    fn division_by_zero_is_invalid() {
        assert_eq!(ev("1 / 0"), None);
    }

    #[test]
    fn rejects_letters_and_unknown() {
        assert_eq!(ev("sin(x)"), None);
        assert_eq!(ev("3 plus 4"), None);
        assert_eq!(ev("hello"), None);
        assert_eq!(ev("20% of 50"), None);
        assert_eq!(ev("$20"), None);
    }

    #[test]
    fn rejects_trailing_op() {
        assert_eq!(ev("3 +"), None);
        assert_eq!(ev("(1 + 2"), None);
        assert_eq!(ev("1 + 2)"), None);
    }

    #[test]
    fn rejects_leftover_tokens() {
        // Two bare numbers with only a space stay ambiguous (no implicit mult
        // between number literals), so the expression is rejected.
        assert_eq!(ev("3 4"), None);
        assert_eq!(ev("3 +"), None);
        assert_eq!(ev("3 )"), None);
    }

    #[test]
    fn bare_number_evaluates() {
        assert_eq!(ev("42"), Some(42.0));
        assert_eq!(ev("  (7) "), Some(7.0));
    }

    #[test]
    fn labelled_single_op() {
        assert_eq!(evaluate_labelled("6 / 2"), Some((3.0, "quotient")));
        assert_eq!(evaluate_labelled("3 * 4"), Some((12.0, "product")));
        assert_eq!(evaluate_labelled("3 + 4"), Some((7.0, "sum")));
        assert_eq!(evaluate_labelled("10 - 4"), Some((6.0, "difference")));
    }

    #[test]
    fn labelled_compound_is_result() {
        assert_eq!(evaluate_labelled("3 + 4 * 2"), Some((11.0, "result")));
        assert_eq!(evaluate_labelled("(1 + 2) * 3"), Some((9.0, "result")));
    }

    #[test]
    fn labelled_bare_number_is_none() {
        assert_eq!(evaluate_labelled("42"), None);
        assert_eq!(evaluate_labelled("(7)"), None);
    }

    #[test]
    fn op_counter() {
        assert_eq!(evaluate("3 + 4 * 2"), Some(11.0));
        assert_eq!(evaluate("42"), Some(42.0));
        assert_eq!(evaluate("sin(x)"), None);
    }

    #[test]
    fn constants() {
        let pi = std::f64::consts::PI;
        let e = std::f64::consts::E;
        assert!((evaluate("pi").unwrap() - pi).abs() < 1e-12);
        assert!((evaluate("e").unwrap() - e).abs() < 1e-12);
        assert!((evaluate("2 * pi").unwrap() - 2.0 * pi).abs() < 1e-12);
        assert!((evaluate("pi / 2").unwrap() - pi / 2.0).abs() < 1e-12);
    }

    #[test]
    fn functions_basic() {
        assert!((evaluate("sin(0)").unwrap() - 0.0).abs() < 1e-12);
        assert!((evaluate("cos(0)").unwrap() - 1.0).abs() < 1e-12);
        assert!((evaluate("sqrt(2)").unwrap() - 2.0_f64.sqrt()).abs() < 1e-12);
        assert!((evaluate("log(100)").unwrap() - 2.0).abs() < 1e-12);
        assert!((evaluate("ln(e)").unwrap() - 1.0).abs() < 1e-12);
        assert!((evaluate("exp(1)").unwrap() - std::f64::consts::E).abs() < 1e-12);
    }

    #[test]
    fn functions_nested_and_combined() {
        assert!((evaluate("sin(pi / 2)").unwrap() - 1.0).abs() < 1e-12);
        assert!((evaluate("sqrt(3 * 3 + 4 * 4)").unwrap() - 5.0).abs() < 1e-12);
        assert!((evaluate("2 * sin(0) + 1").unwrap() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn function_requires_parens() {
        // Bare function name with no argument list is not a valid expression.
        assert_eq!(evaluate("sin"), None);
        assert_eq!(evaluate("sin 5"), None);
        assert_eq!(evaluate("sin(5"), None); // unbalanced
    }

    #[test]
    fn invalid_domains_rejected() {
        assert_eq!(evaluate("sqrt(-1)"), None);
        assert_eq!(evaluate("ln(0)"), None);
        assert_eq!(evaluate("log(-5)"), None);
    }

    #[test]
    fn unknown_identifiers_rejected() {
        // Anything outside the known set routes to the NL classifier.
        assert_eq!(evaluate("sin(x)"), None);
        assert_eq!(evaluate("alpha + 1"), None);
        assert_eq!(evaluate("foo(2)"), None);
        assert_eq!(evaluate("plus"), None);
    }

    #[test]
    fn labelled_constants_and_functions() {
        // A lone constant/function is a real result, not a bare number.
        let (v, label) = evaluate_labelled("pi").unwrap();
        assert!((v - std::f64::consts::PI).abs() < 1e-12);
        assert_eq!(label, "result");
        let (v, label) = evaluate_labelled("sqrt(2)").unwrap();
        assert_eq!(label, "result");
        assert!((v - 2.0_f64.sqrt()).abs() < 1e-12);
        // A single multiply with a constant keeps its op label.
        let (_, label) = evaluate_labelled("2 * pi").unwrap();
        assert_eq!(label, "product");
    }
}
