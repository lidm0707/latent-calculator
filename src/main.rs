//! LatCal terminal — read a natural-language command, print the answer.
//!
//! Uses the fused pipeline: the neuro-symbolic analytical transformer maps
//! natural-language operation words + single-digit arithmetic, with the
//! rule-based engine as fallback for richer inputs (currency, percent, …).

use std::io::{self, BufRead, Write};

use latent_calculator::{Calculator, ParseError};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut lines = stdin.lock().lines();

    loop {
        let _ = write!(out, "> ");
        let _ = out.flush();
        let line = match lines.next() {
            Some(Ok(l)) => l,
            _ => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.eq_ignore_ascii_case("exit") || trimmed.eq_ignore_ascii_case("quit") {
            break;
        }
        match Calculator::parse(trimmed) {
            Ok(answer) => {
                let _ = writeln!(out, "{}", answer.to_sentence());
            }
            Err(e) => {
                let _ = writeln!(out, "{}", error_message(e));
            }
        }
        let _ = out.flush();
    }
}

fn error_message(e: ParseError) -> &'static str {
    match e {
        ParseError::NotMath => "that doesn't look like a math question",
        ParseError::Unknown => "sorry, I could not understand that",
    }
}
