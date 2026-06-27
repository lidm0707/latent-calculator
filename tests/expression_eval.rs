//! Integration tests: the full-expression evaluator vs the NL classifier.
//!
//! Pure arithmetic expressions are now evaluated with precedence, parens, and
//! unary minus; everything else still routes through the NL classifier.

use latent_calculator::Calculator;
use latent_calculator::expr;

const PI: f64 = std::f64::consts::PI;

#[test]
fn precedence_mul_before_add() {
    assert_eq!(
        Calculator::parse("3 + 4 * 2").unwrap().to_sentence(),
        "result is 11"
    );
    assert_eq!(
        Calculator::parse("2 * 3 + 4").unwrap().to_sentence(),
        "result is 10"
    );
}

#[test]
fn precedence_div_before_sub() {
    assert_eq!(
        Calculator::parse("10 - 6 / 2").unwrap().to_sentence(),
        "result is 7"
    );
}

#[test]
fn parentheses_override_precedence() {
    assert_eq!(
        Calculator::parse("(3 + 4) * 2").unwrap().to_sentence(),
        "result is 14"
    );
    assert_eq!(
        Calculator::parse("(1 + 2) * (3 + 4)")
            .unwrap()
            .to_sentence(),
        "result is 21"
    );
}

#[test]
fn nested_parens() {
    assert_eq!(
        Calculator::parse("((1 + 2) * 3) + 4")
            .unwrap()
            .to_sentence(),
        "result is 13"
    );
}

#[test]
fn unary_minus_keeps_binary_op_label() {
    // One binary op (the '+'); the leading '-' is unary, not counted.
    assert_eq!(
        Calculator::parse("-3 + 2").unwrap().to_sentence(),
        "sum is -1"
    );
    assert_eq!(
        Calculator::parse("-(2 + 3)").unwrap().to_sentence(),
        "sum is -5"
    );
}

#[test]
fn left_associative_division() {
    assert_eq!(
        Calculator::parse("100 / 4 / 5").unwrap().to_sentence(),
        "result is 5"
    );
    assert_eq!(
        Calculator::parse("10 / 2 / 5").unwrap().to_sentence(),
        "result is 1"
    );
}

#[test]
fn single_op_keeps_specific_label() {
    assert_eq!(
        Calculator::parse("3 + 4").unwrap().to_sentence(),
        "sum is 7"
    );
    assert_eq!(
        Calculator::parse("12 / 3").unwrap().to_sentence(),
        "quotient is 4"
    );
    assert_eq!(
        Calculator::parse("3 * 4").unwrap().to_sentence(),
        "product is 12"
    );
    assert_eq!(
        Calculator::parse("10 - 4").unwrap().to_sentence(),
        "difference is 6"
    );
}

#[test]
fn decimals_and_unicode_times() {
    assert_eq!(
        Calculator::parse("0.5 + 0.25").unwrap().to_sentence(),
        "sum is 0.75"
    );
    assert_eq!(
        Calculator::parse("3 × 4").unwrap().to_sentence(),
        "product is 12"
    );
}

#[test]
fn expr_rejects_non_expressions() {
    // The non-stealing guarantee: anything that is not a complete arithmetic
    // expression is left for the NL classifier. Checked at the expr boundary.
    assert!(expr::evaluate_labelled("1 / 0").is_none()); // div by zero
    assert!(expr::evaluate_labelled("sin(x + 0.001)").is_none());
    assert!(expr::evaluate_labelled("lim x->0").is_none());
    assert!(expr::evaluate_labelled("3 +").is_none()); // trailing op
    assert!(expr::evaluate_labelled("5 plus 3").is_none()); // word op
    assert!(expr::evaluate_labelled("20% of 50").is_none()); // percent/word
    assert!(expr::evaluate_labelled("42").is_none()); // bare number -> NL Single
}

#[test]
fn nl_inputs_still_route_to_classifier() {
    assert_eq!(
        Calculator::parse("5 plus 3").unwrap().to_sentence(),
        "sum is 8"
    );
    assert_eq!(
        Calculator::parse("3 times 4").unwrap().to_sentence(),
        "product is 12"
    );
    assert_eq!(
        Calculator::parse("20% of 50").unwrap().to_sentence(),
        "result is 10"
    );
}

#[test]
fn constants_evaluate() {
    let a = Calculator::parse("pi").unwrap();
    assert!((a.value - PI).abs() < 1e-9);
    assert_eq!(a.to_sentence(), "result is 3.141592654");
    let b = Calculator::parse("2 * pi").unwrap();
    assert!((b.value - 2.0 * PI).abs() < 1e-9);
    assert_eq!(b.to_sentence(), "product is 6.283185307");
}

#[test]
fn functions_evaluate() {
    assert_eq!(
        Calculator::parse("sin(0)").unwrap().to_sentence(),
        "result is 0"
    );
    assert_eq!(
        Calculator::parse("sqrt(2)").unwrap().to_sentence(),
        "result is 1.414213562"
    );
    assert_eq!(
        Calculator::parse("log(100)").unwrap().to_sentence(),
        "result is 2"
    );
    assert!((Calculator::parse("sin(pi / 2)").unwrap().value - 1.0).abs() < 1e-9);
    assert!((Calculator::parse("sqrt(3 * 3 + 4 * 4)").unwrap().value - 5.0).abs() < 1e-9);
}

#[test]
fn reject_dont_guess_on_unsolvable_expressions() {
    // The headline fix: these used to silently return a lone number.
    assert!(Calculator::parse("sin(x + 0.001)").is_err());
    assert!(Calculator::parse("sqrt(-1)").is_err());
    assert!(Calculator::parse("ln(0)").is_err());
    assert!(Calculator::parse("log(-5)").is_err());
    // A singular limit (0/0) is honestly rejected rather than guessed.
    assert!(Calculator::parse("lim x->0 of 1 / x").is_err());
    assert!(Calculator::parse("lim x->0 of sin(x) / x").is_err());
    assert!(Calculator::parse("alpha + 1").is_err());
    assert!(Calculator::parse("foo(2)").is_err());
}

#[test]
fn numerical_limit_by_substitution() {
    // `lim VAR -> POINT of EXPR` evaluates EXPR at POINT.
    assert_eq!(
        Calculator::parse("lim x->0 of sin(x)")
            .unwrap()
            .to_sentence(),
        "limit is 0"
    );
    assert_eq!(
        Calculator::parse("lim x->2 of x ^ 2")
            .unwrap()
            .to_sentence(),
        "limit is 4"
    );
    assert_eq!(
        Calculator::parse("lim x->0 of 1").unwrap().to_sentence(),
        "limit is 1"
    );
    // The forward-difference approximation of cos(0).
    let fwd = Calculator::parse("lim x->0 of (sin(x + 0.001) - sin(x)) / 0.001").unwrap();
    assert!((fwd.value - 1.0).abs() < 1e-2);
}

#[test]
fn nl_grammar_not_broken_by_gate() {
    // The conservative gate must keep real natural-language math working.
    assert_eq!(
        Calculator::parse("I have 2 dogs and one is die")
            .unwrap()
            .to_sentence(),
        "difference is 1 dog"
    );
    assert_eq!(
        Calculator::parse("2 buy 1").unwrap().to_sentence(),
        "sum is 3"
    );
    assert_eq!(
        Calculator::parse("3 time each item 20$ total")
            .unwrap()
            .to_sentence(),
        "total is 60$"
    );
}

// ── Plan 07: power, more functions, implicit multiplication ─────────────

#[test]
fn power_end_to_end() {
    assert_eq!(
        Calculator::parse("2 ^ 3").unwrap().to_sentence(),
        "result is 8"
    );
    assert_eq!(
        Calculator::parse("2 ^ 3 ^ 2").unwrap().to_sentence(),
        "result is 512"
    );
    assert_eq!(
        Calculator::parse("-2 ^ 2").unwrap().to_sentence(),
        "result is -4"
    );
    assert_eq!(
        Calculator::parse("2 ^ -1").unwrap().to_sentence(),
        "result is 0.5"
    );
    assert_eq!(
        Calculator::parse("2 * 3 ^ 2").unwrap().to_sentence(),
        "result is 18"
    );
}

#[test]
fn more_functions_end_to_end() {
    assert_eq!(
        Calculator::parse("abs(-5)").unwrap().to_sentence(),
        "result is 5"
    );
    assert_eq!(
        Calculator::parse("floor(2.9)").unwrap().to_sentence(),
        "result is 2"
    );
    assert_eq!(
        Calculator::parse("ceil(2.1)").unwrap().to_sentence(),
        "result is 3"
    );
    assert_eq!(
        Calculator::parse("asin(1)").unwrap().to_sentence(),
        "result is 1.570796327"
    );
    assert_eq!(
        Calculator::parse("atan(1)").unwrap().to_sentence(),
        "result is 0.785398163"
    );
}

#[test]
fn implicit_multiplication_end_to_end() {
    assert_eq!(
        Calculator::parse("2(3)").unwrap().to_sentence(),
        "product is 6"
    );
    assert_eq!(
        Calculator::parse("2 pi").unwrap().to_sentence(),
        "product is 6.283185307"
    );
    assert_eq!(
        Calculator::parse("(2)(3)").unwrap().to_sentence(),
        "product is 6"
    );
    assert_eq!(
        Calculator::parse("2(3 + 1)").unwrap().to_sentence(),
        "result is 8"
    );
    assert_eq!(
        Calculator::parse("2(-3)").unwrap().to_sentence(),
        "product is -6"
    );
}

#[test]
fn subtraction_not_implicit_multiplication() {
    // `2 - 3` is subtraction; `2(-3)` is implicit multiplication.
    assert_eq!(
        Calculator::parse("2 - 3").unwrap().to_sentence(),
        "difference is -1"
    );
}

#[test]
fn two_bare_numbers_stay_ambiguous() {
    // No implicit multiplication between two number literals — stays Unknown.
    assert!(Calculator::parse("20 30").is_err());
}

#[test]
fn malformed_power_rejected() {
    // A dangling `^` is not guessed at.
    assert!(Calculator::parse("2 ^").is_err());
    assert!(Calculator::parse("2 ^ ^ 3").is_err());
}
