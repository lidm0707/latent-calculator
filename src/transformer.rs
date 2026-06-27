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
        // Numerical limit via closed-form substitution: `lim VAR -> POINT of EXPR`
        // is evaluated by substituting POINT for VAR and running the expression
        // evaluator. No symbolic math — a singular limit (e.g. sin(x)/x at 0)
        // produces 0/0 and is honestly rejected as `NotMath`.
        if let Some(limit) = try_limit(&expanded) {
            let value = limit?;
            return Ok(Answer {
                value,
                label: "limit",
                currency: None,
                side: CurrencySide::Suffix,
                unit: None,
            });
        }
        // Pure-arithmetic fast path: a complete expression with ≥1 operator
        // (precedence, parens, unary minus). Self-gates via the lexer — anything
        // with letters, currency, or percent falls through to the NL classifier.
        if let Some((value, label)) = crate::expr::evaluate_labelled(&expanded) {
            return Ok(Answer {
                value,
                label,
                currency: None,
                side: CurrencySide::Suffix,
                unit: None,
            });
        }
        // Reject don't-guess: if the input looks like a structured math
        // expression (operators, parens, a known function, or a calculus term)
        // but the evaluator could not solve it, do NOT let the NL classifier
        // rescue a lone number out of the soup. Say "not math" instead.
        if looks_structured(&expanded) {
            return Err(ParseError::NotMath);
        }
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

// ── latent vector + linear attention (hand-set, no training) ──
//
// `features` embeds the latent `State` into a real vector φ ∈ R^N. Each
// operation class owns a hand-set weight row; `attend` scores every
// *applicable* class by the dot product w·φ and takes the argmax. A tiny
// per-class prior (the Bias feature) carries the legacy priority order as a
// deterministic tie-breaker. No learned weights — φ and W are hand-set.

#[derive(Clone, Copy, PartialEq, Eq)]
enum OpClass {
    PercentPrice = 0,
    PercentOf = 1,
    Average = 2,
    TotalCost = 3,
    Unary = 4,
    Root = 5,
    Arith = 6,
    Sum = 7,
    Single = 8,
    Unknown = 9,
}
const N_CLASSES: usize = 10;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Feature {
    HasTax = 0,
    HasDiscount = 1,
    HasPercent = 2,
    HasOf = 3,
    HasAvg = 4,
    HasTotal = 5,
    HasSum = 6,
    HasRoot = 7,
    HasExplicitOp = 8,
    HasNlOp = 9,
    HasUnary = 10,
    HasPrice = 11,
    HasQty = 12,
    HasNumber = 13,
    TwoPlusNumbers = 14,
    OneValue = 15,
    TwoPlusValues = 16,
    Bias = 17,
}
const N_FEATURES: usize = 18;

const WEIGHTS: [[f64; N_FEATURES]; N_CLASSES] = [
    [
        3.0, 3.0, 3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.10,
    ], // PercentPrice
    [
        0.0, 0.0, 3.0, 3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.09,
    ], // PercentOf
    [
        0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 1.0, 0.08,
    ], // Average
    [
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 3.0, 3.0, 0.0, 0.0, 0.0, 0.0, 0.07,
    ], // TotalCost
    [
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.06,
    ], // Unary
    [
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.05,
    ], // Root
    [
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 3.0, 3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.04,
    ], // Arith
    [
        0.0, 0.0, 0.0, 0.0, 0.0, 3.0, 3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.03,
    ], // Sum
    [
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.02,
    ], // Single
    [
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.01,
    ], // Unknown
];

fn bit(b: bool) -> f64 {
    if b { 1.0 } else { 0.0 }
}

fn features(s: &State) -> [f64; N_FEATURES] {
    let values = value_list(s);
    let mut f = [0.0; N_FEATURES];
    f[Feature::HasTax as usize] = bit(s.has_tax);
    f[Feature::HasDiscount as usize] = bit(s.has_discount);
    f[Feature::HasPercent as usize] = bit(!s.percents.is_empty());
    f[Feature::HasOf as usize] = bit(s.has_of);
    f[Feature::HasAvg as usize] = bit(s.has_avg);
    f[Feature::HasTotal as usize] = bit(s.has_total);
    f[Feature::HasSum as usize] = bit(s.has_sum);
    f[Feature::HasRoot as usize] = bit(!s.roots.is_empty());
    f[Feature::HasExplicitOp as usize] = bit(!s.ops.is_empty());
    f[Feature::HasNlOp as usize] = bit(s.nl_op.is_some());
    f[Feature::HasUnary as usize] = bit(s.unary.is_some());
    f[Feature::HasPrice as usize] = bit(!s.prices.is_empty());
    f[Feature::HasQty as usize] = bit(!s.quantities.is_empty());
    f[Feature::HasNumber as usize] = bit(!s.numbers.is_empty());
    f[Feature::TwoPlusNumbers as usize] = bit(s.numbers.len() >= 2);
    f[Feature::OneValue as usize] = bit(values.len() == 1);
    f[Feature::TwoPlusValues as usize] = bit(values.len() >= 2);
    f[Feature::Bias as usize] = 1.0;
    f
}

fn score(class: OpClass, phi: &[f64; N_FEATURES]) -> f64 {
    WEIGHTS[class as usize]
        .iter()
        .zip(phi.iter())
        .map(|(w, x)| w * x)
        .sum()
}

fn attend(s: &State) -> Latent {
    let phi = features(s);
    let candidates: [(OpClass, Option<Latent>); N_CLASSES] = [
        (OpClass::PercentPrice, try_percent_price(s)),
        (OpClass::PercentOf, try_percent_of(s)),
        (OpClass::Average, try_average(s)),
        (OpClass::TotalCost, try_total_cost(s)),
        (OpClass::Unary, try_unary(s)),
        (OpClass::Root, try_root(s)),
        (OpClass::Arith, try_arith(s)),
        (OpClass::Sum, try_sum(s)),
        (OpClass::Single, try_single(s)),
        (OpClass::Unknown, Some(Latent::Unknown)),
    ];
    candidates
        .into_iter()
        .filter_map(|(c, opt)| opt.map(|l| (c, l)))
        .max_by(|(ca, _), (cb, _)| score(*ca, &phi).total_cmp(&score(*cb, &phi)))
        .map(|(_, l)| l)
        .unwrap_or(Latent::Unknown)
}

fn try_average(s: &State) -> Option<Latent> {
    if !s.has_avg || s.numbers.is_empty() {
        return None;
    }
    Some(Latent::Average {
        values: s.numbers.clone(),
    })
}

fn try_total_cost(s: &State) -> Option<Latent> {
    if s.quantities.is_empty() || s.prices.is_empty() || !s.ops.is_empty() {
        return None;
    }
    Some(total_cost(s))
}

fn try_unary(s: &State) -> Option<Latent> {
    let (op, implicit) = s.unary?;
    let v = first_value(s)?;
    Some(Latent::Arith {
        op,
        values: vec![v, implicit],
        currency: s.currency,
        side: s.side,
    })
}

fn try_root(s: &State) -> Option<Latent> {
    let (index, radicand) = s.roots.first().copied()?;
    if !s.ops.is_empty() || s.nl_op.is_some() || s.unary.is_some() {
        return None;
    }
    Some(Latent::Root { index, radicand })
}

fn try_arith(s: &State) -> Option<Latent> {
    let op = s.ops.first().copied().or(s.nl_op)?;
    let values = value_list(s);
    if values.len() < 2 {
        return None;
    }
    Some(Latent::Arith {
        op,
        values,
        currency: s.currency,
        side: s.side,
    })
}

fn try_sum(s: &State) -> Option<Latent> {
    if !(s.has_total || s.has_sum) {
        return None;
    }
    let values = value_list(s);
    if values.len() < 2 {
        return None;
    }
    Some(Latent::Arith {
        op: ArithOp::Add,
        values,
        currency: s.currency,
        side: s.side,
    })
}

fn try_single(s: &State) -> Option<Latent> {
    match value_list(s).as_slice() {
        [v] => Some(Latent::Single {
            value: *v,
            currency: s.currency,
            side: s.side,
        }),
        [] if !s.prices.is_empty() => Some(Latent::Single {
            value: s.prices[0].0,
            currency: s.currency,
            side: s.side,
        }),
        _ => None,
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

// ── Numerical limit by substitution ─────────────────────────
//
// `lim VAR -> POINT of EXPR` (or `limit`, or the `→` arrow) is evaluated by
// substituting POINT for VAR in EXPR and running the arithmetic evaluator.
// This is the only honest thing a modelless numeric calculator can do with a
// limit: it gives the value of the expression at the point. A singular limit
// (0/0, ln(0), ...) yields no value and is rejected as `NotMath`.

/// Returns `Some(Ok(value))` if `s` is a limit form that evaluates to a number,
/// `Some(Err(NotMath))` if it is a limit form that cannot be evaluated (e.g.
/// 0/0 at the point), and `None` if `s` is not a limit form at all.
fn try_limit(s: &str) -> Option<Result<f64, ParseError>> {
    let (var, point, expr) = parse_limit_form(s)?;
    let substituted = substitute_var(expr, var, point);
    match crate::expr::evaluate(&substituted) {
        Some(v) => Some(Ok(v)),
        None => Some(Err(ParseError::NotMath)),
    }
}

/// Structural parse of `lim VAR -> POINT of EXPR`. Returns `(var, point, expr)`
/// as slices borrowed from `s`, or `None` when the prefix does not match. `VAR`
/// must be an alphabetic run that is not a known function/constant, otherwise it
/// would corrupt the substituted expression (e.g. `lim e -> 0 of e`).
fn parse_limit_form(s: &str) -> Option<(&str, &str, &str)> {
    let s = s.trim();
    let b = s.as_bytes();
    // Keyword: "limit" or "lim", case-insensitive, followed by whitespace.
    let kw = if b.len() >= 5 && b[..5].eq_ignore_ascii_case(b"limit") {
        5
    } else if b.len() >= 3 && b[..3].eq_ignore_ascii_case(b"lim") {
        3
    } else {
        return None;
    };
    let mut i = skip_ws(b, kw);
    if i == kw {
        return None; // keyword not followed by whitespace (e.g. "limbo")
    }
    // Variable: an ASCII letter run. Must not collide with a known ident.
    let var_start = i;
    while i < b.len() && b[i].is_ascii_alphabetic() {
        i += 1;
    }
    if i == var_start || crate::expr::is_known_ident(&s[var_start..i]) {
        return None;
    }
    let var = &s[var_start..i];
    i = skip_ws(b, i);
    // Arrow: "->" or the unicode "→".
    if s[i..].starts_with("->") {
        i += 2;
    } else if s[i..].starts_with('→') {
        i += '→'.len_utf8();
    } else {
        return None;
    }
    i = skip_ws(b, i);
    // Limit point: the next non-whitespace run (number, signed number, or a
    // constant like `pi`/`e`). Validated later by the expression evaluator.
    let pt_start = i;
    while i < b.len() && !b[i].is_ascii_whitespace() {
        i += 1;
    }
    if i == pt_start {
        return None;
    }
    let point = &s[pt_start..i];
    i = skip_ws(b, i);
    // "of" keyword, case-insensitive, followed by whitespace.
    if i + 2 > b.len() || !b[i..i + 2].eq_ignore_ascii_case(b"of") {
        return None;
    }
    let after_of = skip_ws(b, i + 2);
    if after_of == i + 2 || after_of >= b.len() {
        return None; // no whitespace after "of", or no expression
    }
    Some((var, point, &s[after_of..]))
}

/// Replace every whole-word occurrence of `var` in `expr` with `(point)`,
/// preserving operator precedence. Word boundaries are ASCII alphanumeric, so
/// `x` inside `exp` or `0.001` is never touched.
fn substitute_var(expr: &str, var: &str, point: &str) -> String {
    let v = var.as_bytes();
    let bytes = expr.as_bytes();
    let mut out = String::with_capacity(expr.len() + 8);
    let mut i = 0;
    while i < bytes.len() {
        let left_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
        let right_ok = i + v.len() <= bytes.len()
            && (i + v.len() == bytes.len() || !bytes[i + v.len()].is_ascii_alphanumeric());
        if left_ok && right_ok && &bytes[i..i + v.len()] == v {
            out.push('(');
            out.push_str(point);
            out.push(')');
            i += v.len();
        } else {
            let c = expr[i..].chars().next().expect("non-empty slice");
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// Structured-expression signals on the raw (post-LaTeX) string. If any are
/// present, the input was attempting to be a math expression/function call;
/// when the evaluator rejects it, the NL classifier must not guess from it.
const STRUCT_SYMBOLS: &[char] = &['+', '-', '/', '*', '×', '÷', '(', ')', '\\'];
const CALCULUS_WORDS: &[&str] = &[
    "lim",
    "limit",
    "derivative",
    "differentiate",
    "integral",
    "integrate",
    "dx",
    "dy",
];
const SUPPORTED_FUNCS: &[&str] = &["sin", "cos", "tan", "sqrt", "log", "ln", "exp"];

fn looks_structured(s: &str) -> bool {
    if s.contains("->") || s.chars().any(|c| STRUCT_SYMBOLS.contains(&c)) {
        return true;
    }
    let lower = s.to_ascii_lowercase();
    for word in lower.split(|c: char| !c.is_ascii_alphanumeric()) {
        if word.is_empty() {
            continue;
        }
        if CALCULUS_WORDS.contains(&word) || SUPPORTED_FUNCS.contains(&word) {
            return true;
        }
    }
    false
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
    fn limit_by_substitution() {
        // Closed-form substitution: lim VAR -> POINT of EXPR == EXPR at POINT.
        assert_eq!(
            run("lim x->0 of sin(x)").unwrap().to_sentence(),
            "limit is 0"
        );
        assert_eq!(
            run("lim x->2 of x ^ 2").unwrap().to_sentence(),
            "limit is 4"
        );
        assert_eq!(run("lim x->0 of 1").unwrap().to_sentence(), "limit is 1");
        // The forward-difference approximation of cos(0): the expression as
        // written evaluates to ~1 at x = 0.
        let fwd = run("lim x->0 of (sin(x + 0.001) - sin(x)) / 0.001").unwrap();
        assert!(fwd.to_sentence().starts_with("limit is "));
        assert!((fwd.value - 1.0).abs() < 1e-2);
    }

    #[test]
    fn limit_accepts_arrow_and_keyword_variants() {
        assert_eq!(run("limit x->3 of x").unwrap().to_sentence(), "limit is 3");
        assert_eq!(run("lim x → 3 of x").unwrap().to_sentence(), "limit is 3");
        assert_eq!(
            run("lim x -> -1 of x + 1").unwrap().to_sentence(),
            "limit is 0"
        );
    }

    #[test]
    fn limit_rejects_singular_and_non_limit_forms() {
        // 0/0 at the point: honest rejection, never a guessed lone number.
        assert_eq!(run("lim x->0 of sin(x) / x"), Err(ParseError::NotMath));
        assert_eq!(run("lim x->0 of 1 / x"), Err(ParseError::NotMath));
        // Variable colliding with a known ident is not treated as a limit.
        assert_eq!(run("lim e->0 of e"), Err(ParseError::NotMath));
        // Malformed limit syntax (no `of`) still hits the calculus gate.
        assert_eq!(run("lim x->0"), Err(ParseError::NotMath));
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

    #[test]
    fn vector_attention_picks_the_right_class() {
        // The latent vector + linear attention must select the same operation the
        // old priority chain did, including the Arith-over-Sum tie-break.
        let phi = features(&embed(&tokenize("I have 2 dogs and one is die")));
        let s = score(OpClass::Arith, &phi) - score(OpClass::Sum, &phi);
        assert!(s > 0.0, "Arith must out-score Sum, got delta {s}");
        assert_eq!(
            run("I have 2 dogs and one is die").unwrap().label,
            "difference"
        );
    }

    /// Zero-dependency micro-bench of the full parse pipeline (tokenize →
    /// embed → gate → vector-attend → decode). Run with:
    ///   cargo test vector_attention_bench -- --ignored --nocapture
    #[test]
    #[ignore]
    fn vector_attention_bench() {
        use std::time::Instant;
        let corpus = [
            "I have 2 dogs and one dog is dead.",
            "10 tax 7%",
            "90 tax 8.8%",
            "5 plus 3",
            "average of 4 8 and 12",
            "3 time each item 20$ total",
            "double 15",
            "\\sqrt[3]{27}",
            "20",
        ];
        const ITERS: usize = 100_000;
        let start = Instant::now();
        let mut acc = 0u64;
        for _ in 0..ITERS {
            for q in corpus {
                if let Ok(a) = Calculator::parse(q) {
                    acc = acc.wrapping_add(a.value.to_bits());
                }
            }
        }
        let elapsed = start.elapsed();
        let queries = ITERS * corpus.len();
        let ns_per = elapsed.as_nanos() as f64 / queries as f64;
        println!("bench: {queries} parses in {elapsed:?} = {ns_per:.0} ns/parse (sink={acc})");
    }
}
