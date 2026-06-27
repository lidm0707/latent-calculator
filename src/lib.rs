//! LatCal — modelless natural-language calculator.
//!
//! Neuro-symbolic: a tokenizer feeds a hand-set latent-space engine that
//! understands natural-language math (operation words, currency, percent,
//! discount/tax) and decodes the result symbolically. No learned weights,
//! zero dependencies, no feature flags.

pub mod tokenizer;
pub mod transformer;

pub use tokenizer::{ArithOp, Currency, CurrencySide, Token};
pub use transformer::{Answer, Calculator, ParseError};
