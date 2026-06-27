//! Integration tests: the spec example, NL subtraction (the dogs example),
//! and the LaTeX arithmetic subset.

use latent_calculator::{Calculator, Currency, CurrencySide};

#[test]
fn spec_terminal_example() {
    let input = "I buy persona5 3time each item 20$ in what is price total";
    let answer = Calculator::parse(input).expect("spec example must parse");
    assert_eq!(answer.value, 60.0);
    assert_eq!(answer.currency, Some(Currency::Dollar));
    assert_eq!(answer.side, CurrencySide::Suffix);
    assert_eq!(answer.to_sentence(), "total is 60$");
}

#[test]
fn prefix_currency_total() {
    let answer = Calculator::parse("3 copies at $20 total").unwrap();
    assert_eq!(answer.to_sentence(), "total is $60");
}

#[test]
fn plain_arithmetic() {
    assert_eq!(
        Calculator::parse("5 plus 3").unwrap().to_sentence(),
        "sum is 8"
    );
    assert_eq!(
        Calculator::parse("10 minus 4").unwrap().to_sentence(),
        "difference is 6"
    );
    assert_eq!(
        Calculator::parse("3 times 4").unwrap().to_sentence(),
        "product is 12"
    );
    assert_eq!(
        Calculator::parse("12 divided 3").unwrap().to_sentence(),
        "quotient is 4"
    );
}

#[test]
fn average() {
    assert_eq!(
        Calculator::parse("average of 4 8 and 12")
            .unwrap()
            .to_sentence(),
        "average is 8"
    );
}

#[test]
fn percent_of() {
    assert_eq!(
        Calculator::parse("20% of 50").unwrap().to_sentence(),
        "result is 10"
    );
}

#[test]
fn discount_and_tax() {
    assert_eq!(
        Calculator::parse("10$ discount 2%").unwrap().to_sentence(),
        "result is 9.8$"
    );
    assert_eq!(
        Calculator::parse("100$ discount 20%")
            .unwrap()
            .to_sentence(),
        "result is 80$"
    );
    assert_eq!(
        Calculator::parse("50$ tax 10%").unwrap().to_sentence(),
        "result is 55$"
    );
}

#[test]
fn natural_language_subtraction_dogs() {
    // The headline NL example: broken English, possession + death verb.
    assert_eq!(
        Calculator::parse("I have 2 dogs and one is die")
            .unwrap()
            .to_sentence(),
        "difference is 1 dog"
    );
}

#[test]
fn latex_frac_times_div() {
    assert_eq!(
        Calculator::parse("\\frac{6}{2}").unwrap().to_sentence(),
        "quotient is 3"
    );
    assert_eq!(
        Calculator::parse("\\frac{100}{4}").unwrap().to_sentence(),
        "quotient is 25"
    );
    assert_eq!(
        Calculator::parse("3 \\times 4").unwrap().to_sentence(),
        "product is 12"
    );
    assert_eq!(
        Calculator::parse("12 \\div 3").unwrap().to_sentence(),
        "quotient is 4"
    );
}

#[test]
fn latex_sqrt() {
    assert_eq!(
        Calculator::parse("\\sqrt{9}").unwrap().to_sentence(),
        "root is 3"
    );
    assert_eq!(
        Calculator::parse("\\sqrt[3]{27}").unwrap().to_sentence(),
        "root is 3"
    );
    // \pi expands to the f64 constant; sqrt snaps to 9 decimals.
    assert_eq!(
        Calculator::parse("\\sqrt{\\pi}").unwrap().to_sentence(),
        "root is 1.772453851"
    );
}
