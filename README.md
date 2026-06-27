# latent-calculator (LatCal)

Modelless natural-language calculator — **no ML weights, no training, zero dependencies**.

A neuro-symbolic latent-space engine: a tokenizer feeds a hand-set forward pass that
understands natural-language math (`buy`, `discount`, `tax`, `double`, currency, percent),
then decodes the result symbolically. Pure Rust, std-only, no feature flags.

> Referenced as `LatCalIx` in Plan 244. "Modelless" = no learned weights anywhere.

---

## Use it in a Dioxus app

Add the crate as a **git dependency** and Dioxus for the web target:

```toml
# Cargo.toml
[dependencies]
latent-calculator = { git = "https://github.com/lidm0707/katgpt-rs" }
dioxus            = { version = "0.6", features = ["web"] }

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
cargo install dioxus-cli --version 0.6

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
| `10$ discount 2%` | `result is 9.8$` | percent-price |
| `2 buy 1` | `sum is 3` | NL word → `+` |
| `double 15` | `product is 30` | NL word → `×2` |
| `why 2 dog die 1` | _not a math question_ | plausibility gate |

### Natural-language operation vocabulary

Compiled into the transformer's op slot (any operand magnitude):

| Word | Operation |
|---|---|
| `buy` `get` `gain` `receive` | `+` |
| `eat` `lose` `give` `take` `spend` `drop` | `−` |
| `double` | `×2` |
| `triple` | `×3` |
| `discount` `off` `sale` `save` | price × (1 − pct/100) |
| `tax` `tip` `vat` | price × (1 + pct/100) |

Structural words: `total`/`price`/`cost`/`sum` (total cost), `average`/`avg`/`mean`, `of`
(percent-of), `and`/`plus` (sum).

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
`Currency`, `CurrencySide`, `Answer`).

### Architecture — 3 files

```
src/
  tokenizer.rs   lexical classification (numbers, currency, percent, ops, count-units)
  transformer.rs neuro-symbolic latent-space engine (the brain)
  main.rs        terminal REPL (optional; not built when used as a dependency)
  lib.rs         thin re-exports
```

Pipeline: `tokens → embed → attend → Latent → decode → Answer`.

- **`embed`** — gathers latent operand slots (quantities, prices, numbers, percents) + flags.
- **`attend`** — reads the operation + operands out of the latent state; NL operation
  words compile into the op slot here.
- **`decode`** — symbolic arithmetic on the `Latent` (arbitrary precision, currency, percent).
- **Plausibility gate** — no math anchor + noise → `NotMath`.

This split is what makes it neuro-symbolic: neural-style understanding (`embed` + `attend`)
selects the operation and operands; `decode` does the arithmetic.

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
> why 2 dog die 1
that doesn't look like a math question
```

## Build & test

```sh
cargo test     -p latent-calculator        # 17 tests (11 unit + 6 integration), no flags
cargo clippy   -p latent-calculator --all-targets
```

## License

MIT
