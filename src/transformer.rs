//! Neuro-symbolic latent-space engine.
//!
//! Pipeline: tokens → [`embed`] (attend to each token's kind/value, gather the
//! latent operand slots) → [`attend`] (read out the operation + operands into a
//! [`Latent`] computation) → [`decode`] (symbolic arithmetic + formatting).
//!
//! The forward pass (`embed` + `attend`) is the "latent space": it understands
//! the natural-language input — which operation, which operands, which currency.
//! `decode` is the symbolic half: it computes the number. No learned weights —
//! the embeddings/attention are hand-set. This split is what makes it
//! neuro-symbolic: neural-style understanding, symbolic computation.
//!
//! Natural-language operation words are compiled into the op slot:
//! - `buy`/`get`/`gain`/`receive` → `+`, `eat`/`lose`/`give`/`take`/`spend`/`drop` → `−`
//! - `double` → `×2`, `triple` → `×3`
//! - `discount`/`off`/`sale`/`save` → price × (1 − pct/100), `tax`/`tip`/`vat` → price × (1 + pct/100)

use crate::tokenizer::{ArithOp, Currency, CurrencySide, Token, is_count_unit, tokenize};

// ── Natural-language operation vocabulary ──────────────────────
const NL_ADD: &[&str] = &["buy", "get", "gain", "receive"];
const NL_SUB: &[&str] = &[
    "eat", "lose", "loses", "lost", "give", "take", "spend", "drop", "die", "died", "dies", "dead",
    "death", "gone", "kill", "killed", "remove", "removed", "fewer",
];
const NL_DISCOUNT: &[&str] = &["discount", "off", "sale", "save"];
const NL_TAX: &[&str] = &["tax", "tip", "vat"];
const MATH_KEYWORDS: &[&str] = &[
    "of",
    "by",
    "and",
    "average",
    "avg",
    "mean",
    "total",
    "altogether",
    "price",
    "cost",
    "sum",
];

// ── Public API ─────────────────────────────────────────────────

/// A computed answer ready to be rendered as a natural-language sentence.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub value: f64,
    pub label: &'static str,
    pub currency: Option<Currency>,
    pub side: CurrencySide,
    pub unit: Option<String>,
}

impl Answer {
    pub fn to_sentence(&self) -> String {
        let num = fmt_num(self.value);
        let core = match (self.currency, self.side) {
            (Some(cur), CurrencySide::Suffix) => format!("{}{}", num, cur.symbol()),
            (Some(cur), CurrencySide::Prefix) => format!("{}{}", cur.symbol(), num),
            (None, _) => num,
        };
        let with_unit = match &self.unit {
            Some(u) => format!("{core} {u}"),
            None => core,
        };
        format!("{} is {}", self.label, with_unit)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    Unknown,
    /// Input does not look like a math command (no math anchor + noise words).
    NotMath,
}

/// Entry point: `Calculator::parse("...")` → `Answer`.
pub struct Calculator;

impl Calculator {
    pub fn parse(input: &str) -> Result<Answer, ParseError> {
        let expanded = crate::latex::expand(input);
        let tokens = tokenize(&expanded);
        let state = embed(&tokens);
        // Plausibility gate: no math anchor surrounded by noise → NotMath.
        if !has_anchor(&state) && has_noise(&tokens) {
            return Err(ParseError::NotMath);
        }
        let latent = attend(&state);
        match decode(&latent) {
            Some((value, label, currency, side)) => Ok(Answer {
                value,
                label,
                currency,
                side,
                unit: pick_noun(&state.units, value),
            }),
            None => Err(ParseError::Unknown),
        }
    }
}

// ── Latent representation (output of the forward pass) ─────────

#[derive(Debug, Clone)]
enum Latent {
    Arith {
        op: ArithOp,
        values: Vec<f64>,
        currency: Option<Currency>,
        side: CurrencySide,
    },
    TotalCost {
        items: Vec<(f64, f64)>,
        currency: Currency,
        side: CurrencySide,
    },
    Average {
        values: Vec<f64>,
    },
    PercentOf {
        rate: f64,
        base: f64,
    },
    PercentPrice {
        price: f64,
        percent: f64,
        dir: PercentDir,
        currency: Option<Currency>,
        side: CurrencySide,
    },
    Single {
        value: f64,
        currency: Option<Currency>,
        side: CurrencySide,
    },
    Root {
        index: f64,
        radicand: f64,
    },
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PercentDir {
    Discount,
    Tax,
}

// ── embed: attend to each token, gather latent operand slots ───

#[derive(Default)]
struct State {
    quantities: Vec<f64>,
    prices: Vec<(f64, Currency)>,
    numbers: Vec<f64>,
    roots: Vec<(f64, f64)>,
    ops: Vec<ArithOp>,             // explicit operator tokens
    nl_op: Option<ArithOp>,        // NL operation word (buy/eat/…) — weaker than explicit ops
    unary: Option<(ArithOp, f64)>, // double/triple → (Mul, implicit operand)
    percents: Vec<f64>,
    has_percent: bool,
    has_of: bool,
    has_avg: bool,
    has_total: bool,
    has_sum: bool,
    has_discount: bool,
    has_tax: bool,
    currency: Option<Currency>,
    side: CurrencySide,
    units: Vec<String>,
}

fn embed(tokens: &[Token<'_>]) -> State {
    let mut s = State::default();
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Token::Number(n) => {
                let next = tokens.get(i + 1);
                if let Some(Token::Times) = next {
                    // "N times M" → multiply; "N times" (no following number) → quantity.
                    if matches!(tokens.get(i + 2), Some(Token::Number(_))) {
                        s.numbers.push(*n);
                        s.ops.push(ArithOp::Mul);
                        i += 2;
                        continue;
                    }
                    s.quantities.push(*n);
                    i += 2;
                    continue;
                }
                if let Some(Token::Word(w)) = next
                    && is_count_unit(w)
                {
                    s.quantities.push(*n);
                    i += 2;
                    continue;
                }
                // capture a trailing count noun: "2 dogs", "5 apples".
                if let Some(Token::Word(w)) = next
                    && is_count_noun(w)
                {
                    s.units.push(w.to_ascii_lowercase());
                }
                s.numbers.push(*n);
            }
            Token::Quantity(q) => s.quantities.push(*q),
            Token::Currency { value, cur, side } => {
                s.prices.push((*value, *cur));
                if s.currency.is_none() {
                    s.currency = Some(*cur);
                    s.side = *side;
                }
            }
            Token::PercentValue(v) => {
                s.percents.push(*v);
                s.has_percent = true;
            }
            Token::Percent => s.has_percent = true,
            Token::Op(op) => s.ops.push(*op),
            Token::Times => s.ops.push(ArithOp::Mul),
            Token::Root { index, radicand } => s.roots.push((*index, *radicand)),
            Token::Word(w) => classify_word(w, &mut s),
        }
        i += 1;
    }
    s
}

/// Map a word token into the latent state: structural cues, percent-price
/// directions, and NL operation words (compiled into the op slot).
fn classify_word(w: &str, s: &mut State) {
    // Recover numbers the strict tokenizer rejected: thousands separators
    // ("30,211") or a number glued to a known keyword ("90tax"). Only commit
    // when the suffix is empty or recognizable, so noise like "2@" stays noise.
    if let Some((num, rest)) = loose_number(w)
        && (rest.is_empty() || is_known_suffix(rest))
    {
        s.numbers.push(num);
        if !rest.is_empty() {
            classify_word(rest, s);
        }
        return;
    }
    let lw = w.to_ascii_lowercase();
    match lw.as_str() {
        "of" => s.has_of = true,
        "by" => {}
        "average" | "avg" | "mean" => s.has_avg = true,
        "total" | "altogether" | "price" | "cost" | "sum" => s.has_total = true,
        "and" => s.has_sum = true,
        w if NL_DISCOUNT.iter().any(|k| w.eq_ignore_ascii_case(k)) => s.has_discount = true,
        w if NL_TAX.iter().any(|k| w.eq_ignore_ascii_case(k)) => s.has_tax = true,
        w if NL_ADD.iter().any(|k| w.eq_ignore_ascii_case(k)) => {
            s.nl_op = s.nl_op.or(Some(ArithOp::Add))
        }
        w if NL_SUB.iter().any(|k| w.eq_ignore_ascii_case(k)) => {
            s.nl_op = s.nl_op.or(Some(ArithOp::Sub))
        }
        "double" => s.unary = Some((ArithOp::Mul, 2.0)),
        "triple" => s.unary = Some((ArithOp::Mul, 3.0)),
        _ => {}
    }
}

/// Read a number out of a word the tokenizer's strict `parse_num` rejected:
/// thousands separators ("30,211") or a number glued to a keyword ("90tax").
/// Returns the value plus the trailing suffix (empty for a pure number).
fn loose_number(w: &str) -> Option<(f64, &str)> {
    let bytes = w.as_bytes();
    let mut end = 0;
    let mut saw_digit = false;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'0'..=b'9' => {
                saw_digit = true;
                end = i + 1;
            }
            b',' | b'.' => {}
            _ => break,
        }
    }
    if !saw_digit {
        return None;
    }
    let cleaned: String = w[..end].chars().filter(|&c| c != ',').collect();
    let v = cleaned.parse::<f64>().ok().filter(|v| v.is_finite())?;
    Some((v, &w[end..]))
}

/// Only split a glued number when what follows is itself a meaningful token,
/// so random punctuation glued to a digit doesn't get promoted into a rate.
fn is_known_suffix(rest: &str) -> bool {
    is_math_keyword(rest)
        || is_count_unit(rest)
        || rest == "%"
        || rest.eq_ignore_ascii_case("percent")
        || rest.eq_ignore_ascii_case("pct")
}

/// A bare alphabetic word that isn't an operator/unit keyword — i.e. a count
/// noun like "dogs" or "apples" that should be echoed in the answer.
fn is_count_noun(w: &str) -> bool {
    !w.is_empty()
        && w.chars().all(|c| c.is_ascii_alphabetic())
        && !is_math_keyword(w)
        && !is_count_unit(w)
}

/// Pick the noun to echo alongside the result: prefer a candidate already in
/// the right grammatical number for `value`, and singularize (drop trailing
/// 's') when the result is exactly 1.
fn pick_noun(units: &[String], value: f64) -> Option<String> {
    if units.is_empty() {
        return None;
    }
    let singular = value == 1.0;
    let preferred = units
        .iter()
        .find(|u| singular != u.ends_with('s'))
        .or_else(|| units.first())?;
    if singular && preferred.ends_with('s') && preferred.len() > 1 {
        Some(preferred[..preferred.len() - 1].to_string())
    } else {
        Some(preferred.clone())
    }
}

// ── attend: read the operation + operands out of the latent state ──

fn attend(s: &State) -> Latent {
    if let Some(c) = try_percent_price(s) {
        return c;
    }
    if let Some(c) = try_percent_of(s) {
        return c;
    }
    if s.has_avg && !s.numbers.is_empty() {
        return Latent::Average {
            values: s.numbers.clone(),
        };
    }
    // TotalCost wins over a bare NL op (so "I buy 3 at 20$ total" stays a total).
    if !s.quantities.is_empty() && !s.prices.is_empty() && s.ops.is_empty() {
        return total_cost(s);
    }
    if let Some((op, implicit)) = s.unary
        && let Some(v) = first_value(s)
    {
        return Latent::Arith {
            op,
            values: vec![v, implicit],
            currency: s.currency,
            side: s.side,
        };
    }
    if let Some((index, radicand)) = s.roots.first().copied()
        && s.ops.is_empty()
        && s.nl_op.is_none()
        && s.unary.is_none()
    {
        return Latent::Root { index, radicand };
    }
    let arith_op = s.ops.first().copied().or(s.nl_op);
    if let Some(op) = arith_op {
        let values = value_list(s);
        if values.len() >= 2 {
            return Latent::Arith {
                op,
                values,
                currency: s.currency,
                side: s.side,
            };
        }
    }
    if (s.has_total || s.has_sum) && value_list(s).len() >= 2 {
        return Latent::Arith {
            op: ArithOp::Add,
            values: value_list(s),
            currency: s.currency,
            side: s.side,
        };
    }
    match value_list(s).as_slice() {
        [v] => Latent::Single {
            value: *v,
            currency: s.currency,
            side: s.side,
        },
        [] if !s.prices.is_empty() => Latent::Single {
            value: s.prices[0].0,
            currency: s.currency,
            side: s.side,
        },
        _ => Latent::Unknown,
    }
}

fn try_percent_price(s: &State) -> Option<Latent> {
    let dir = if s.has_discount {
        PercentDir::Discount
    } else if s.has_tax {
        PercentDir::Tax
    } else {
        return None;
    };

    // Base: prefer a currency price, otherwise the first bare number — so
    // "90 tax 8.8%" works without a currency symbol.
    let (price, currency) = match s.prices.first() {
        Some(&(p, cur)) => (p, Some(cur)),
        None => (*s.numbers.first()?, None),
    };

    // Rate: prefer an explicit percent token ("7%"), otherwise read it from
    // the next bare number so "10 tax 7" reads as 10 + 7%.
    let percent = match s.percents.first() {
        Some(&p) => p,
        None => {
            let rate_idx = if s.prices.is_empty() { 1 } else { 0 };
            *s.numbers.get(rate_idx)?
        }
    };

    Some(Latent::PercentPrice {
        price,
        percent,
        dir,
        currency,
        side: s.side,
    })
}

fn try_percent_of(s: &State) -> Option<Latent> {
    if s.percents.is_empty() || !s.has_of || s.numbers.is_empty() {
        return None;
    }
    Some(Latent::PercentOf {
        rate: s.percents[0],
        base: s.numbers[0],
    })
}

fn total_cost(s: &State) -> Latent {
    let items: Vec<(f64, f64)> = match (s.quantities.len(), s.prices.len()) {
        (q, p) if q == p => s
            .quantities
            .iter()
            .copied()
            .zip(s.prices.iter().map(|(p, _)| *p))
            .collect(),
        (1, 1) => vec![(s.quantities[0], s.prices[0].0)],
        _ => s
            .quantities
            .iter()
            .flat_map(|&q| s.prices.iter().map(move |&(p, _)| (q, p)))
            .collect(),
    };
    Latent::TotalCost {
        items,
        currency: s.currency.unwrap_or(Currency::Dollar),
        side: s.side,
    }
}

fn first_value(s: &State) -> Option<f64> {
    s.numbers
        .first()
        .copied()
        .or(s.prices.first().map(|(p, _)| *p))
}

fn value_list(s: &State) -> Vec<f64> {
    if !s.numbers.is_empty() {
        s.numbers.clone()
    } else {
        s.prices.iter().map(|(p, _)| *p).collect()
    }
}

// ── decode: symbolic arithmetic on the latent computation ──────

fn decode(l: &Latent) -> Option<(f64, &'static str, Option<Currency>, CurrencySide)> {
    match l {
        Latent::TotalCost {
            items,
            currency,
            side,
        } => {
            let total: f64 = items.iter().map(|(q, p)| q * p).sum();
            Some((total, "total", Some(*currency), *side))
        }
        Latent::Arith {
            op,
            values,
            currency,
            side,
        } => {
            let v = apply(op, values)?;
            Some((v, label_of(*op), *currency, *side))
        }
        Latent::Average { values } => {
            if values.is_empty() {
                return None;
            }
            let sum: f64 = values.iter().sum();
            Some((
                sum / values.len() as f64,
                "average",
                None,
                CurrencySide::Suffix,
            ))
        }
        Latent::PercentOf { rate, base } => {
            Some((rate / 100.0 * base, "result", None, CurrencySide::Suffix))
        }
        Latent::PercentPrice {
            price,
            percent,
            dir,
            currency,
            side,
        } => {
            let factor = match dir {
                PercentDir::Discount => 1.0 - percent / 100.0,
                PercentDir::Tax => 1.0 + percent / 100.0,
            };
            Some((price * factor, "result", *currency, *side))
        }
        Latent::Single {
            value,
            currency,
            side,
        } => Some((*value, "result", *currency, *side)),
        Latent::Root { index, radicand } => {
            let v = radicand.powf(1.0 / index);
            if v.is_finite() {
                Some((v, "root", None, CurrencySide::Suffix))
            } else {
                None
            }
        }
        Latent::Unknown => None,
    }
}

fn apply(op: &ArithOp, values: &[f64]) -> Option<f64> {
    let (first, rest) = values.split_first()?;
    Some(match op {
        ArithOp::Add => rest.iter().fold(*first, |a, b| a + b),
        ArithOp::Sub => rest.iter().fold(*first, |a, b| a - b),
        ArithOp::Mul => rest.iter().fold(*first, |a, b| a * b),
        ArithOp::Div => rest.iter().fold(*first, |a, b| a / b),
    })
}

fn label_of(op: ArithOp) -> &'static str {
    match op {
        ArithOp::Add => "sum",
        ArithOp::Sub => "difference",
        ArithOp::Mul => "product",
        ArithOp::Div => "quotient",
    }
}

/// Snap float noise (9 decimals) then format: integers without a decimal point.
pub fn fmt_num(v: f64) -> String {
    let snapped = (v * 1e9).round() / 1e9;
    if snapped.fract() == 0.0 && snapped.abs() < 1e15 {
        format!("{}", snapped as i64)
    } else {
        format!("{snapped}")
    }
}

// ── plausibility gate helpers ──────────────────────────────────

fn has_anchor(s: &State) -> bool {
    s.has_total
        || s.has_avg
        || s.has_percent
        || s.has_discount
        || s.has_tax
        || !s.ops.is_empty()
        || s.nl_op.is_some()
        || s.unary.is_some()
        || !s.prices.is_empty()
        || !s.quantities.is_empty()
        || !s.percents.is_empty()
}

fn has_noise(tokens: &[Token<'_>]) -> bool {
    tokens.iter().any(|t| match t {
        Token::Word(w) => !is_math_keyword(w),
        _ => false,
    })
}

fn is_math_keyword(w: &str) -> bool {
    MATH_KEYWORDS.iter().any(|k| w.eq_ignore_ascii_case(k))
        || NL_ADD.iter().any(|k| w.eq_ignore_ascii_case(k))
        || NL_SUB.iter().any(|k| w.eq_ignore_ascii_case(k))
        || NL_DISCOUNT.iter().any(|k| w.eq_ignore_ascii_case(k))
        || NL_TAX.iter().any(|k| w.eq_ignore_ascii_case(k))
        || matches!(w.to_ascii_lowercase().as_str(), "double" | "triple")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &str) -> Result<Answer, ParseError> {
        Calculator::parse(s)
    }

    #[test]
    fn spec_example() {
        assert_eq!(
            run("I buy persona5 3time each item 20$ in what is price total")
                .unwrap()
                .to_sentence(),
            "total is 60$"
        );
    }

    #[test]
    fn arithmetic() {
        assert_eq!(run("5 plus 3").unwrap().to_sentence(), "sum is 8");
        assert_eq!(run("10 minus 4").unwrap().to_sentence(), "difference is 6");
        assert_eq!(run("3 times 4").unwrap().to_sentence(), "product is 12");
        assert_eq!(run("12 divided 3").unwrap().to_sentence(), "quotient is 4");
    }

    #[test]
    fn average_and_percent() {
        assert_eq!(
            run("average of 4 8 and 12").unwrap().to_sentence(),
            "average is 8"
        );
        assert_eq!(run("20% of 50").unwrap().to_sentence(), "result is 10");
    }

    #[test]
    fn discount_and_tax() {
        assert_eq!(
            run("10$ discount 2%").unwrap().to_sentence(),
            "result is 9.8$"
        );
        assert_eq!(
            run("100$ discount 20%").unwrap().to_sentence(),
            "result is 80$"
        );
        assert_eq!(run("50$ tax 10%").unwrap().to_sentence(), "result is 55$");
    }

    #[test]
    fn nl_operation_words() {
        // NL words map into the op slot — now for any magnitude, not just single digits.
        assert_eq!(run("2 buy 1").unwrap().to_sentence(), "sum is 3");
        assert_eq!(run("20 buy 15").unwrap().to_sentence(), "sum is 35");
        assert_eq!(run("5 eat 2").unwrap().to_sentence(), "difference is 3");
        assert_eq!(run("double 5").unwrap().to_sentence(), "product is 10");
        assert_eq!(run("double 15").unwrap().to_sentence(), "product is 30");
        assert_eq!(run("triple 3").unwrap().to_sentence(), "product is 9");
    }

    #[test]
    fn rejects_non_math_noise() {
        assert_eq!(run("why 2 dog and 1 cat"), Err(ParseError::NotMath));
        assert_eq!(run("the quick brown fox"), Err(ParseError::NotMath));
    }

    #[test]
    fn terse_pure_math_still_works() {
        assert_eq!(run("5 and 3").unwrap().to_sentence(), "sum is 8");
        assert_eq!(run("20").unwrap().to_sentence(), "result is 20");
    }

    #[test]
    fn ambiguous_terse_is_unknown() {
        assert_eq!(run("20 30"), Err(ParseError::Unknown));
    }

    #[test]
    fn natural_language_subtraction() {
        // "I have 2 dogs and one died" -> 2 - 1 = 1 remains, noun retained.
        assert_eq!(
            run("I have 2 dogs and one is die").unwrap().to_sentence(),
            "difference is 1 dog"
        );
        assert_eq!(
            run("5 apples lost 2").unwrap().to_sentence(),
            "difference is 3 apples"
        );
    }

    #[test]
    fn noun_retained_in_answer() {
        // The result carries the subject noun, singular when the count is 1.
        assert_eq!(
            run("I have 2 dogs and one dog is dead.")
                .unwrap()
                .to_sentence(),
            "difference is 1 dog"
        );
    }

    #[test]
    fn tax_and_discount_on_bare_numbers() {
        // Bare base (no currency symbol): the core transcript bug.
        assert_eq!(run("10 tax 7%").unwrap().to_sentence(), "result is 10.7");
        assert_eq!(run("90 tax 8.8%").unwrap().to_sentence(), "result is 97.92");
        assert_eq!(
            run("iphone 30211 vat 10%").unwrap().to_sentence(),
            "result is 33232.1"
        );
        assert_eq!(
            run("5 discount 10%").unwrap().to_sentence(),
            "result is 4.5"
        );
    }

    #[test]
    fn tax_without_percent_symbol() {
        // "10 tax 7" reads the trailing number as the rate.
        assert_eq!(run("10 tax 7").unwrap().to_sentence(), "result is 10.7");
    }

    #[test]
    fn thousands_separators_and_glued_keyword() {
        // Recovered inside the transformer (tokenizer untouched).
        assert_eq!(
            run("iphone 30,211 vat 10%").unwrap().to_sentence(),
            "result is 33232.1"
        );
        assert_eq!(run("90tax 8.8%").unwrap().to_sentence(), "result is 97.92");
    }

    #[test]
    fn latex_arithmetic() {
        assert_eq!(run("\\frac{6}{2}").unwrap().to_sentence(), "quotient is 3");
        assert_eq!(run("3 \\times 4").unwrap().to_sentence(), "product is 12");
        assert_eq!(run("\\sqrt{9}").unwrap().to_sentence(), "root is 3");
        assert_eq!(run("\\sqrt[3]{27}").unwrap().to_sentence(), "root is 3");
    }
}
