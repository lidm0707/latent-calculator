# latent-calculator (LatCal)

Modelless natural-language calculator — **no ML weights, no training, zero dependencies**.

A neuro-symbolic latent-space engine: a tokenizer feeds a hand-set forward pass that
understands natural-language math (`buy`, `discount`, `tax`, `double`, currency, percent),
a practical subset of **LaTeX** (`\frac`, `\sqrt`, `\times`, …), then decodes the
result symbolically. Pure Rust, std-only, no feature flags.

> Referenced as `LatCalIx` in Plan 244. "Modelless" = no learned weights anywhere.

---

## Use it in a Dioxus app

Add the crate as a **git dependency** and Dioxus for the web target:

```toml
# Cargo.toml
[dependencies]
latent-calculator = { git = "https://github.com/lidm0707/katgpt-rs" }
dioxus            = { version = "0.7", features = ["web"] }

# Optional: pin to a specific commit or branch
# latent-calculator = { git = "https://github.com/lidm0707/katgpt-rs", rev = "<commit-sha>" }
```

> The crate lives at `crates/latent-calculator` inside that repo. Cargo resolves the
> package by name across the workspace, so the path is not needed.

### Minimal example — live NL calculator

The result is **derived directly from the input signal** (no effect chains):

```rust
use dioxus::prelude::*;
use latent_calculator::{Calculator, ParseError};

fn main() {
    dioxus::launch(App);
}

fn App() -> Element {
    // single source of truth
    let mut input = use_signal(|| String::from("3 time each item 20$ total"));

    // derived state — recomputes when `input` changes
    let result = use_memo(move || match Calculator::parse(&input.read()) {
        Ok(answer) => answer.to_sentence(),
        Err(ParseError::NotMath) => "that doesn't look like a math question".to_string(),
        Err(ParseError::Unknown) => "sorry, I could not understand that".to_string(),
    });

    rsx! {
        div { style: "font-family: system-ui; max-width: 560px; margin: 3rem auto; padding: 1rem;",
            h1 { "LatCal — natural-language calculator" }
            input {
                value: "{input}",
                oninput: move |e| input.set(e.value()),
                style: "width: 100%; padding: 0.6rem; font-size: 1rem; box-sizing: border-box;",
                placeholder: "try: 10$ discount 2%"
            }
            p { style: "font-size: 1.6rem; margin-top: 1rem; min-height: 2rem;", "{result}" }
        }
    }
}
```

### Run the web app

```sh
# install the Dioxus CLI once
cargo install dioxus-cli --version 0.7

# serve with hot-reload
dx serve --platform web
```

### SSR / fullstack

`latent-calculator` is std-only and allocation-free over borrowed tokens, so it runs
fine on the server. In a fullstack Dioxus app, parse in a server function and stream
the sentence to the client — no WASM bundle cost for the math.

---

## What it understands

| Input | Output | Kind |
|---|---|---|
| `3 time each item 20$ total` | `total is 60$` | total cost |
| `5 plus 3` | `sum is 8` | arithmetic |
| `3 + 4 * 2` | `result is 11` | expression (precedence) |
| `(1 + 2) * 3` | `result is 9` | expression (parens) |
| `-3 + 2` | `sum is -1` | expression (unary minus) |
| `sin(pi / 2)` | `result is 1` | function call |
| `sqrt(2)` | `result is 1.414213562` | function call |
| `2 ^ 3 ^ 2` | `result is 512` | power (right-assoc) |
| `-2 ^ 2` | `result is -4` | power binds tighter than unary `-` |
| `2 pi` | `product is 6.283185307` | implicit multiplication |
| `2(3)` | `product is 6` | implicit multiplication |
| `abs(-5)` | `result is 5` | function call |
| `10$ discount 2%` | `result is 9.8$` | percent-price |
| `2 buy 1` | `sum is 3` | NL word → `+` |
| `I have 2 dogs and one is die` | `difference is 1 dog` | NL word → `−` (death/loss verb), noun retained |
| `double 15` | `product is 30` | NL word → `×2` |
| `\frac{6}{2}` | `quotient is 3` | LaTeX division |
| `\sqrt[3]{27}` | `root is 3` | LaTeX k-th root |
| `why 2 dog and 1 cat` | _not a math question_ | plausibility gate |
| `sin(x + 0.001)` | _not a math question_ | reject don't-guess (unsolvable: free var) |
| `lim x->0 of sin(x)` | `limit is 0` | limit by substitution |
| `lim x->2 of x^2` | `limit is 4` | limit by substitution |
| `lim x->0 of (sin(x+0.001)-sin(x))/0.001` | `limit is 0.999999833` | limit (≈ cos(0)) |
| `lim x->0 of sin(x)/x` | _not a math question_ | reject (0/0 singular, no CAS) |
| `derivative of x^2` | _not a math question_ | reject (calculus, no symbolic) |

### What it does NOT do (yet)

LatCal is a numeric calculator, not a computer-algebra system. It will tell you when
it can't handle something instead of guessing:

- **No symbolic math** — free variables (`x`, `alpha`) and equation solving are out of scope (a `lim` variable is substituted, not solved symbolically).
- **No symbolic calculus** — `derivative`/`integral` are rejected. `lim VAR -> POINT of EXPR` is supported **by closed-form substitution** (it evaluates `EXPR` at `POINT`); singular limits like `sin(x)/x` at 0 produce `0/0` and are honestly rejected as `not a math question`.
- **No `÷`-free juxtaposition** — `2(3)` *is* implicit multiplication, but two bare number literals `2 3` stay ambiguous (use `2 * 3`).

Functions supported: `sin` `cos` `tan` `asin` `acos` `atan` `sqrt` `log` (base-10) `ln` `exp` `abs` `floor` `ceil`. Constants: `pi` `e`. Operators: `+ - * / ^` (and `× ÷`), with implicit multiplication for `2 pi`, `2(3)`, `(2)(3)`. Power is right-associative and binds tighter than unary minus (`-2^2 = -4`).

### Natural-language operation vocabulary

Compiled into the transformer's op slot (any operand magnitude):

| Word | Operation |
|---|---|
| `buy` `get` `gain` `receive` | `+` |
| `eat` `lose` `loses` `lost` `give` `take` `spend` `drop` `die` `died` `dies` `dead` `death` `gone` `kill` `killed` `remove` `removed` `fewer` | `−` |
| `double` | `×2` |
| `triple` | `×3` |
| `discount` `off` `sale` `save` | price × (1 − pct/100) |
| `tax` `tip` `vat` | price × (1 + pct/100) |

Structural words: `total`/`price`/`cost`/`sum` (total cost), `average`/`avg`/`mean`, `of`
(percent-of), `and`/`plus` (sum).

### LaTeX subset

A preprocessor ([`src/latex.rs`](src/latex.rs)) rewrites LaTeX arithmetic into the
forms the tokenizer already understands, so LaTeX and plain English share the same
latent engine:

| LaTeX | Expansion | |
|---|---|---|
| `\frac{a}{b}` `\dfrac` `\tfrac` | `( a ) / ( b )` | division |
| `\times` `\cdot` | `×` | multiply |
| `\div` | `÷` | divide |
| `\sqrt{n}` | `root{2,n}` | square root |
| `\sqrt[k]{n}` | `root{k,n}` | k-th root |
| `\pi` | `3.141592653589793` | constant |
| `\$` `\%` `\{` `\}` | `$` `%` `{` `}` | escaped literals |
| `\,` `\;` `\:` `\!` | space | thin/hair spacing |
| `\left(` `\right)` | `( )` | sized delimiters (qualifier dropped) |

Unknown commands are passed through verbatim, so LaTeX-free input is unaffected.
_Note: pure arithmetic expressions are evaluated by a dedicated recursive-descent
parser with full operator precedence, parentheses, and unary minus. Juxtaposition-
as-multiplication (`2\frac{1}{2}`) is not modeled._

---

## Library API

```rust
use latent_calculator::Calculator;

let a = Calculator::parse("2 buy 1").unwrap();          // → "sum is 3"
let b = Calculator::parse("10$ discount 2%").unwrap();  // → "result is 9.8$"

println!("{}", a.to_sentence());
```

`ParseError` is either `NotMath` (no math signal) or `Unknown` (couldn't compute).
See [`src/lib.rs`](src/lib.rs) for the full re-export list (`Token`, `ArithOp`,
`Currency`, `CurrencySide`, `Answer`). LaTeX expansion is available directly as
`latent_calculator::latex::expand`.

### Architecture — 5 files

```
src/
  expr.rs        recursive-descent arithmetic evaluator (precedence, parens, unary minus)
  tokenizer.rs   lexical classification (numbers, currency, percent, ops, count-units, roots)
  latex.rs       LaTeX arithmetic preprocessor (\frac / \sqrt / \times / \div / \pi / escapes)
  transformer.rs neuro-symbolic latent-space engine (the brain)
  main.rs        terminal REPL (optional; not built when used as a dependency)
  lib.rs         thin re-exports
```

Pipeline: `input → latex::expand → limit (substitute & eval) → expr fast-path (if pure arithmetic) → tokens → embed → attend → Latent → decode → Answer`.

- **`latex::expand`** — rewrites the LaTeX subset into tokenizer-friendly forms (no-op when no `\`).
- **limit** — `lim VAR -> POINT of EXPR` is solved by substituting `POINT` for `VAR` and running the expr evaluator. No symbolic math: a singular limit (`sin(x)/x` at 0) yields `0/0` and is rejected as `NotMath`. Self-gates — only matches the `lim`/`limit` prefix with a non-colliding variable.
- **`expr` fast-path** — if the expanded input is a complete arithmetic expression with ≥1
  operator, a recursive-descent parser evaluates it with precedence, parentheses, and unary
  minus. It self-gates (the lexer rejects letters/`%`/currency), so natural-language input
  falls through to the latent engine unchanged. A single binary op keeps its specific label
  (`sum`/`difference`/`product`/`quotient`); a compound expression reports `result`.
- **`embed`** — gathers latent operand slots (quantities, prices, numbers, percents, roots) + flags.
- **`attend`** — **linear attention, hand-set (no training)**: `features` embeds the
  latent `State` into a vector `φ ∈ R^18`; every applicable operation class is
  scored by a dot product `w·φ` with a hand-set weight row, and the argmax wins.
  A tiny per-class prior carries the legacy priority order as a tie-breaker.
- **`decode`** — symbolic arithmetic on the `Latent` (arithmetic, currency, percent, roots).
- **Plausibility gate** — no math anchor + noise → `NotMath`.

This split is what makes it neuro-symbolic: a **real** latent vector + linear
attention (`embed` + `attend`) selects the operation and operands; `decode` does
the arithmetic. No learned weights anywhere — `φ` and the weight matrix are hand-set.

---

## Terminal REPL (optional binary)

```sh
cargo run -p latent-calculator
```

```
> 3 time each item 20$ total
total is 60$
> 10$ discount 2%
result is 9.8$
> I have 2 dogs and one is die
difference is 1 dog
> \sqrt[3]{27}
root is 3
```

## Build & test

```sh
cargo test     -p latent-calculator        # 97 tests, no flags
cargo clippy   -p latent-calculator --all-targets
```

## License

MIT
