//! Tokenizer — turn a natural-language string into typed tokens.
//!
//! Pure lexical classification: numbers, currency (`$20`/`20$`), count-units
//! (`3time`/`3x`), percents (`20%`), explicit operator words (`plus`/`minus`/…),
//! and symbols. Word tokens borrow from the input (zero-copy).
//!
//! Semantic understanding (which operation, which operands) happens in the
//! transformer's latent space — this module only tokenizes.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Currency {
    Dollar,
    Euro,
    Pound,
    Yen,
}

impl Currency {
    pub fn symbol(self) -> &'static str {
        match self {
            Currency::Dollar => "$",
            Currency::Euro => "€",
            Currency::Pound => "£",
            Currency::Yen => "¥",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CurrencySide {
    #[default]
    Suffix,
    Prefix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
}

const OP_SPLIT_CHARS: &[char] = &['+', '-', '*', '/', '×', '÷'];

#[derive(Debug, Clone, PartialEq)]
pub enum Token<'a> {
    Number(f64),
    Currency {
        value: f64,
        cur: Currency,
        side: CurrencySide,
    },
    Quantity(f64),
    PercentValue(f64),
    Percent,
    Times,
    Op(ArithOp),
    /// k-th root produced by LaTeX `\sqrt[k]{n}` (square root when k = 2).
    Root {
        index: f64,
        radicand: f64,
    },
    Word(&'a str),
}

pub fn tokenize<'a>(input: &'a str) -> Vec<Token<'a>> {
    let mut out = Vec::new();
    for raw in input.split_whitespace() {
        for piece in split_ops(raw) {
            if let Some(tok) = classify(piece) {
                out.push(tok);
            }
        }
    }
    out
}

fn split_ops(raw: &str) -> Vec<&str> {
    if !raw.bytes().any(|b| b.is_ascii_digit()) {
        return vec![raw];
    }
    let mut pieces = Vec::new();
    let bytes = raw.as_bytes();
    let mut start = 0;
    for (i, &b) in bytes.iter().enumerate() {
        if OP_SPLIT_CHARS.iter().any(|&c| c as u8 == b) {
            if i > start {
                pieces.push(&raw[start..i]);
            }
            pieces.push(&raw[i..i + 1]);
            start = i + 1;
        }
    }
    if start < raw.len() {
        pieces.push(&raw[start..]);
    }
    if pieces.is_empty() { vec![raw] } else { pieces }
}

fn classify(s: &str) -> Option<Token<'_>> {
    let s = trim_punct(s);
    if s.is_empty() {
        return None;
    }
    if let Some(op) = op_token(s) {
        return Some(Token::Op(op));
    }
    if s == "%" || s.eq_ignore_ascii_case("percent") || s.eq_ignore_ascii_case("pct") {
        return Some(Token::Percent);
    }
    if let Some((v, cur, side)) = currency_token(s) {
        return Some(Token::Currency {
            value: v,
            cur,
            side,
        });
    }
    if let Some(v) = percent_value_token(s) {
        return Some(Token::PercentValue(v));
    }
    if let Some((index, radicand)) = root_token(s) {
        return Some(Token::Root { index, radicand });
    }
    if let Some(q) = quantity_token(s) {
        return Some(Token::Quantity(q));
    }
    if let Some(v) = number_token(s) {
        return Some(Token::Number(v));
    }
    if is_times_word(s) {
        return Some(Token::Times);
    }
    Some(Token::Word(s))
}

fn trim_punct(s: &str) -> &str {
    let bytes = s.as_bytes();
    let mut start = 0;
    while start < bytes.len() && bytes[start] == b'(' {
        start += 1;
    }
    let mut end = bytes.len();
    while end > start
        && matches!(
            bytes[end - 1],
            b'.' | b',' | b';' | b':' | b'!' | b'?' | b')'
        )
    {
        end -= 1;
    }
    &s[start..end]
}

fn op_token(s: &str) -> Option<ArithOp> {
    match s {
        "+" | "plus" | "add" => Some(ArithOp::Add),
        "-" | "minus" | "subtract" | "less" => Some(ArithOp::Sub),
        "*" | "×" | "multiply" | "multiplied" | "product" => Some(ArithOp::Mul),
        "/" | "÷" | "divide" | "divided" => Some(ArithOp::Div),
        _ => None,
    }
}

fn currency_token(s: &str) -> Option<(f64, Currency, CurrencySide)> {
    let first = s.chars().next()?;
    let last = s.chars().last()?;
    if let Some(cur) = currency_of(first) {
        let v = parse_num(&s[first.len_utf8()..])?;
        return Some((v, cur, CurrencySide::Prefix));
    }
    if let Some(cur) = currency_of(last) {
        let v = parse_num(&s[..s.len() - last.len_utf8()])?;
        return Some((v, cur, CurrencySide::Suffix));
    }
    None
}

fn root_token(s: &str) -> Option<(f64, f64)> {
    let inner = s.strip_prefix("root{")?.strip_suffix('}')?;
    let mut it = inner.split(',');
    let index = parse_num(it.next()?)?;
    let radicand = parse_num(it.next()?)?;
    if it.next().is_some() || index <= 0.0 {
        return None;
    }
    Some((index, radicand))
}

fn percent_value_token(s: &str) -> Option<f64> {
    if let Some(rest) = s.strip_suffix('%') {
        return parse_num(rest);
    }
    for suffix in ["percent", "pct"] {
        if let Some(rest) = s.strip_suffix(suffix).filter(|r| !r.is_empty())
            && let Some(v) = parse_num(rest)
        {
            return Some(v);
        }
    }
    None
}

fn quantity_token(s: &str) -> Option<f64> {
    let (num, rest) = split_num_rest(s)?;
    if rest.is_empty() {
        return None;
    }
    if is_count_unit(rest) {
        parse_num(num)
    } else {
        None
    }
}

fn number_token(s: &str) -> Option<f64> {
    if let Some(v) = parse_num(s) {
        return Some(v);
    }
    number_word(s)
}

fn split_num_rest(s: &str) -> Option<(&str, &str)> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
        i += 1;
    }
    if i == 0 {
        return None;
    }
    Some((&s[..i], &s[i..]))
}

fn parse_num(s: &str) -> Option<f64> {
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn currency_of(c: char) -> Option<Currency> {
    match c {
        '$' => Some(Currency::Dollar),
        '€' => Some(Currency::Euro),
        '£' => Some(Currency::Pound),
        '¥' => Some(Currency::Yen),
        _ => None,
    }
}

pub fn is_count_unit(s: &str) -> bool {
    matches!(
        s.to_ascii_lowercase().as_str(),
        "time"
            | "times"
            | "x"
            | "copy"
            | "copies"
            | "piece"
            | "pieces"
            | "pcs"
            | "unit"
            | "units"
            | "qty"
    )
}

fn is_times_word(s: &str) -> bool {
    matches!(s.to_ascii_lowercase().as_str(), "time" | "times" | "x")
}

const NUMBER_WORDS: &[(&str, f64)] = &[
    ("zero", 0.0),
    ("one", 1.0),
    ("two", 2.0),
    ("three", 3.0),
    ("four", 4.0),
    ("five", 5.0),
    ("six", 6.0),
    ("seven", 7.0),
    ("eight", 8.0),
    ("nine", 9.0),
    ("ten", 10.0),
    ("eleven", 11.0),
    ("twelve", 12.0),
    ("thirteen", 13.0),
    ("fourteen", 14.0),
    ("fifteen", 15.0),
    ("sixteen", 16.0),
    ("seventeen", 17.0),
    ("eighteen", 18.0),
    ("nineteen", 19.0),
    ("twenty", 20.0),
    ("thirty", 30.0),
    ("forty", 40.0),
    ("fifty", 50.0),
    ("sixty", 60.0),
    ("seventy", 70.0),
    ("eighty", 80.0),
    ("ninety", 90.0),
    ("hundred", 100.0),
    ("thousand", 1000.0),
    ("million", 1_000_000.0),
];

fn number_word(s: &str) -> Option<f64> {
    NUMBER_WORDS
        .iter()
        .find(|(w, _)| w.eq_ignore_ascii_case(s))
        .map(|(_, v)| *v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_spec_example() {
        let toks = tokenize("I buy persona5 3time each item 20$ in what is price total");
        assert!(toks.contains(&Token::Quantity(3.0)));
        assert!(toks.contains(&Token::Currency {
            value: 20.0,
            cur: Currency::Dollar,
            side: CurrencySide::Suffix
        }));
    }

    #[test]
    fn inline_ops_split() {
        let toks = tokenize("5+3");
        assert_eq!(
            toks,
            vec![
                Token::Number(5.0),
                Token::Op(ArithOp::Add),
                Token::Number(3.0)
            ]
        );
    }

    #[test]
    fn prefix_currency() {
        let toks = tokenize("$20");
        assert_eq!(
            toks,
            vec![Token::Currency {
                value: 20.0,
                cur: Currency::Dollar,
                side: CurrencySide::Prefix
            }]
        );
    }
}
