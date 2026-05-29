# AVEN — Agent Vector Expression Notation

**AVEN is a programming language built for AI agents to write, read, and patch code.**

Most programming languages are designed so that humans can type them quickly. AVEN makes the opposite tradeoff: it is explicit, canonical, and machine-legible — optimised for the AI-in-the-loop workflow where code is generated, reviewed, patched, and re-reviewed thousands of times.

---

## The Problem

When an AI agent writes Python or JavaScript, it has to choose between equivalent forms:

```python
# All of these mean the same thing — the agent picks one arbitrarily
lambda x: x * 2
def double(x): return x * 2
def double(x):
    return x * 2
```

Arbitrary choice means inconsistent output, larger diffs, and harder review. Worse, there is no standard way to say "this expression needs human review before deployment" — you can use a comment, but comments are stripped by tooling and carry no enforcement.

## The AVEN Approach

AVEN has **one spelling for every construct**. Every function is `@fn`. Every return is `@ret`. Every binary op uses prefix notation with explicit parens. There is no sugar, no shorthand, no optionality. An agent always produces the same token sequence for the same program.

Beyond syntax, AVEN has **AI-native AST nodes** baked into the grammar:

- `@uncertain` — marks an expression as needing review. Transparent at runtime; blocked by the verifier at deploy time.
- `@intent` — an AST-level docstring queryable by selector, not a comment.
- `@diff` — a semantic patch format that targets AST nodes by path, not line numbers.

And **capability-gated imports**: `@use [read] from fs` — each import declares exactly which operations the module is allowed to perform. An auditing agent can determine the full side-effect surface from the `@use` declarations alone.

---

## What Is in This Repo

This is the **AVEN seed interpreter** — Milestone 1 of 7, written in Rust. It implements enough of the language to run programs, verify `@uncertain` annotations, and serve as the backend for [aven-guard](https://github.com/roeeash/aven_guard).

| Component | File | What it does |
|---|---|---|
| Lexer | `src/lexer.rs` | Tokenises AVEN source; handles `@` sigils, `#` symbols, `::` separator |
| AST | `src/ast.rs` | Expression and node type definitions |
| Parser | `src/parser.rs` | Recursive-descent parser; produces typed AST |
| Evaluator | `src/eval.rs` | Tree-walking interpreter with closures and environment |
| Type checker | `src/typechecker.rs` | Validates type annotations (full enforcement in M2) |
| Formatter | `src/fmt.rs` | Canonical pretty-printer; `aven fmt` |
| CLI | `src/main.rs` | REPL + `verify`, `intent`, `check-uncertainty` subcommands |
| Tests | `tests/integration.rs` | End-to-end test suite |

---

## Quick Start

### Requirements

- Rust toolchain: `rustc 1.70+`, `cargo`

### Build

```bash
git clone https://github.com/roeeash/aven_lang
cd aven_lang
cargo build --release
# binary at: target/release/aven
```

### Run the REPL

```bash
cargo run --bin aven
```

```
aven> (+ 2 3)
5
aven> @let x :: 10
10
aven> (+ x 5)
15
```

### Run Tests

```bash
cargo test
```

---

## Language by Example

### Arithmetic (prefix, explicit parens)

```aven
(+ 2 3)          ; => 5
(* 3 4)          ; => 12
(+ (* 2 3) 4)    ; => 10
```

### Functions

```aven
@fn square :: n:Int -> Int
  @ret (* n n)

@call square 7   ; => 49
```

### Conditionals

```aven
@if @true @then "yes" @else "no"   ; => yes
```

### Let Bindings

```aven
@let x :: 10
(+ x 5)          ; => 15
```

### I/O

```aven
@io.write "Hello, AVEN!"   ; prints: Hello, AVEN!
```

### `@uncertain` — mark code for review

```aven
@uncertain (+ 2 2)   ; evaluates to 4 at runtime, but blocks deployment via verifier
```

### `@intent` — AST-level documentation

```aven
@intent "compute the square of the input"
@fn square :: n:Int -> Int
  @ret (* n n)
```

### Capability-gated imports (M4+)

```aven
@use [read] from fs       ; can read files, cannot write
@use [get] from http      ; can make GET requests, cannot POST
```

---

## CLI Subcommands

### REPL

```bash
aven
```

### Verify a `.aven` file (used by aven-guard)

Parses, type-checks, and checks for `@uncertain` annotations. Outputs JSON.

```bash
aven verify myfile.aven
# {"file": "myfile.aven", "pass": true, "errors": []}
```

```bash
# File containing @uncertain:
aven verify uncertain.aven
# {"file": "uncertain.aven", "pass": false, "errors": [{"stage": "uncertainty", "message": "..."}]}
```

### Check for `@uncertain` annotations

```bash
aven check-uncertainty myfile.aven
# 3:5: @uncertain at path '/fn greet/body'
```

### Extract `@intent` docstrings

```bash
aven intent myfile.aven
# /fn square  "compute the square of the input"
```

---

## How aven-guard Uses This

[aven-guard](https://github.com/roeeash/aven_guard) is a Python SDK that wraps this binary. It walks Python ASTs, detects dangerous patterns (`eval`, `subprocess`, `os.remove`, etc.), translates them to AVEN `@uncertain` annotations, and calls `aven verify` to produce a structured pass/fail result. The AVEN binary is the enforcement layer; aven-guard is the Python integration.

---

## Milestone Roadmap

| Milestone | Status | What lands |
|---|---|---|
| **M1 — Core interpreter** | ✅ This repo | Lex, parse, eval, `verify` CLI, `@uncertain`/`@intent` as AST nodes |
| **M2 — Type system** | Planned | Full bidirectional type checker; effect arrows (`->` / `-!>` / `-~>` etc.) |
| **M3 — Control flow** | Planned | `@match`, `@err`, typed `#ok \| #err` results |
| **M4 — Module system** | Planned | `@mod`, `@use`, capability verification at import boundaries |
| **M5 — `@diff` engine** | Planned | Selector-addressed AST patches; `.avenpatch` files; atomic batch diffs |
| **M6 — Stdlib** | Planned | `aven/std/io`, `fs`, `http`, `json`, `math`, `collections` |
| **M7 — Self-hosting prep** | Planned | `aven fmt`, span-aware errors, full spec coverage |

See [ROADMAP.md](ROADMAP.md) for detail and [AVEN_SPEC.md](AVEN_SPEC.md) for the full language specification.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).
