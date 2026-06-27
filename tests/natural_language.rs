//! Integration test: the user's spec example must round-trip to "total is 60$".

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
    // `price discount pct%` ⇒ final price after discount.
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
    // `price tax pct%` ⇒ final price after tax.
    assert_eq!(
        Calculator::parse("50$ tax 10%").unwrap().to_sentence(),
        "result is 55$"
    );
}
