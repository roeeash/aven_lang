# AVEN — Agent Vector Expression Notation

**AVEN is a programming language built for AI agents to write, read, and patch code safely.**

---

## Why AVEN

AI agents generating code face three problems that existing languages don't solve:

1. **No safe import model.** An agent that calls `import os` can delete files, spawn processes, or exfiltrate data — and nothing in the language stops it. There's no way to say "this module may only read, not write."
2. **No structured way to patch code.** When an agent needs to change a function, it rewrites the whole file or diffs by line number. Line diffs break on any formatting change; full rewrites discard context.
3. **No enforcement of "needs review."** An agent can leave a comment saying something is uncertain, but comments carry no runtime or deploy-time weight — tooling strips them.

AVEN solves all three at the language level.

---

## Capability-Gated Imports

Every import in AVEN declares exactly which capabilities the module is allowed to exercise:

```aven
@use [read] from fs          ; can open and read files — cannot write or delete
@use [get] from http         ; can make GET requests — cannot POST or mutate
@use [read, write] from fs   ; explicit write grant required for mutation
```

An auditing agent, a CI gate, or a human reviewer can determine the **entire side-effect surface** of a program by reading the `@use` declarations — without executing anything. A module that tries to write when it only declared `[read]` is rejected at the import boundary.

This is the difference between "hope the agent didn't call `rm`" and having a verifiable contract.

---

## `@diff` — AST-addressed patches

When an agent needs to change code, it shouldn't rewrite the whole file. AVEN's `@diff` format targets nodes by **AST path**, not line number:

```aven
@diff /fn square/body
  @ret (* n n n)   ; change square to cube
```

This patch applies correctly regardless of whitespace, comments, or reformatting. Multiple diffs can be batched into a single `.avenpatch` file and applied atomically. Line-number diffs rot the moment the file is touched; AST diffs don't.

_(Full `@diff` engine ships in M5 — the grammar and AST path format are defined in M1.)_

---

## `@uncertain` — enforced review gates

When an agent isn't confident about a piece of code, it can mark it:

```aven
@uncertain (os.remove "/etc/passwd")
```

`@uncertain` is a **first-class AST node**, not a comment. It evaluates normally at runtime (transparent to execution), but `aven verify` rejects any file containing it. You cannot deploy uncertain code without resolving every annotation — the verifier enforces this, not a human process.

[aven-guard](https://github.com/roeeash/aven_guard) uses this: it walks Python ASTs, detects dangerous patterns (`eval`, `subprocess`, `os.remove`, etc.), and translates them to `@uncertain` annotations for the verifier.

```bash
aven verify myfile.aven
# {"file": "myfile.aven", "pass": false,
#  "errors": [{"stage": "uncertainty", "message": "@uncertain at line 3"}]}
```

---

## `@intent` — queryable documentation

`@intent` is an AST-level docstring. Unlike comments, it's part of the AST — tooling can extract, index, and query it by selector:

```aven
@intent "compute the square of n"
@fn square :: n:Int -> Int
  @ret (* n n)
```

```bash
aven intent myfile.aven
# /fn square   "compute the square of n"
```

An agent generating code writes `@intent` alongside the function. An auditing agent queries intents to verify a module does what it claims — without reading the implementation.

---

## Unambiguous Grammar

AVEN has one spelling for every construct. No sugar, no shorthand, no optionality:

```aven
@fn square :: n:Int -> Int     ; every function: @fn name :: args -> return
  @ret (* n n)                 ; every return: @ret expr

@if @true @then "yes" @else "no"   ; every conditional: @if cond @then a @else b

@let x :: 10                   ; every binding: @let name :: value
(+ x 5)                        ; every binary op: prefix with explicit parens
```

An agent always produces the same token sequence for the same program. Diffs are minimal. Reviews are consistent. Two agents working on the same codebase won't produce structurally different outputs for the same logic.

---

## Quick Start

### Requirements

Rust toolchain: `rustc 1.70+`, `cargo`

### Build

```bash
git clone https://github.com/roeeash/aven_lang
cd aven_lang
cargo build --release
# binary at: target/release/aven
```

### REPL

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

### Tests

```bash
cargo test
```

---

## Compiling AVEN Programs

The AVEN-written compiler lives in `aven-core/`. It is run by the seed interpreter (the Rust binary). All pipeline stages are concatenated and fed to `aven run`.

### Run a program through the full pipeline

```bash
# Concatenate all compiler stages, then run main.aven on your source file
cat aven-core/lexer.aven \
    aven-core/parser.aven \
    aven-core/check.aven \
    aven-core/module.aven \
    aven-core/diff.aven \
    aven-core/eval.aven \
    aven-core/driver.aven \
    aven-core/main.aven > /tmp/aven-compiler.aven

aven run /tmp/aven-compiler.aven -- yourprogram.aven
```

The pipeline is: **tokenize → parse → type-check → evaluate**. If type checking fails, an `ERROR:...` string is returned and evaluation is skipped.

### Inspect intermediate stages

```bash
# Tokens only (lex stage)
aven run /tmp/aven-compiler.aven -- --tokens yourprogram.aven

# AST only (lex + parse)
aven run /tmp/aven-compiler.aven -- --ast yourprogram.aven

# Type-check only (lex + parse + check)
aven run /tmp/aven-compiler.aven -- --check yourprogram.aven
```

### Apply an AST patch

```bash
# Apply a selector-addressed diff to an AST
aven run /tmp/aven-compiler.aven -- --patch "fn square" "(FnDef cube ...)" yourprogram.aven
```

### Example

Given `hello.aven`:

```aven
@let x :: 42
(+ x 1)
```

```bash
$ aven run /tmp/aven-compiler.aven -- hello.aven
43
```

---

## CLI Subcommands (seed interpreter)

| Command | What it does |
|---|---|
| `aven` | Interactive REPL |
| `aven verify <file>` | Parse, type-check, reject `@uncertain`. Outputs JSON. |
| `aven check-uncertainty <file>` | List all `@uncertain` annotations with AST paths |
| `aven intent <file>` | Extract all `@intent` docstrings by selector |
| `aven fmt <file>` | Canonical formatter (idempotent) |

---

## What Is in This Repo

This repo contains the **AVEN seed interpreter** (Rust, M1–M7) and the **AVEN-written compiler** (Stage 2, `aven-core/`).

| Component | File | What it does |
|---|---|---|
| Lexer | `src/lexer.rs` | Tokenises AVEN source; handles `@` sigils, `#` symbols, `::` separator |
| AST | `src/ast.rs` | Expression and node type definitions |
| Parser | `src/parser.rs` | Recursive-descent parser; produces typed AST |
| Evaluator | `src/eval.rs` | Tree-walking interpreter with closures and environment |
| Type checker | `src/typechecker.rs` | Structural annotation validator (full enforcement in M2) |
| Formatter | `src/fmt.rs` | Canonical pretty-printer |
| CLI | `src/main.rs` | REPL + `verify`, `intent`, `check-uncertainty` subcommands |
| Tests | `tests/integration.rs` | End-to-end test suite (503+ passing) |

**Stage 2 — AVEN-written compiler (`aven-core/`)**

The compiler is entirely written in AVEN and runs on the Rust seed interpreter. Each file is one pipeline stage. They are concatenated in order to form the full compiler.

| Stage | File | What it does |
|---|---|---|
| S2.1 | `aven-core/lexer.aven` | Tokenises source text into a `\n`-joined token stream (`Ident x`, `Integer 42`, `Eof`, …) |
| S2.2 | `aven-core/parser.aven` | Recursive-descent parser; converts token stream to `\n`-joined S-expression AST |
| S2.3 | `aven-core/check.aven` | Type checker; threads an environment of `name:type` bindings; returns `PASS` or `ERROR:…` |
| S2.4 | `aven-core/module.aven` | Module resolver; checks that each `@use` declares only capabilities the registry grants |
| S2.5 | `aven-core/diff.aven` | `@diff` engine; selector-addressed AST replacement (`fn name` / `let name` selectors) |
| S2.6 | `aven-core/eval.aven` | Tree-walking evaluator; threads an env of `name:value` bindings; returns the final value as a string |
| S2.7 | `aven-core/driver.aven` | Wires all stages into `run :: source → value`; short-circuits on type errors |
| S2.7 | `aven-core/main.aven` | Entry point; delegates to `run` |

---

## Milestone Roadmap

| Milestone | Status | What lands |
|---|---|---|
| **M1 — Core interpreter** | ✅ Complete | Lex, parse, eval, source spans, `NodeId`, `verify` CLI, `@uncertain`/`@intent` as AST nodes |
| **M2 — Type system** | ✅ Complete | Bidirectional type checker, orthogonal `EffectSet`, 8 effect arrows, `@cap` markers, capability gating stubs |
| **M3 — Control flow** | ✅ Complete | `@match` with pattern binding, `@err` values, typed `#ok \| #err` results, `@intent` indexing |
| **M4 — Module system** | ✅ Complete | `@mod`, `@use`, `@pub`, capability verification, DAG cycle detection, topo-ordered type checking, real `@ctx` API |
| **M5 — `@diff` engine** | ✅ Complete | Selector-addressed AST patches (`@replace`/`@insert`/`@delete`/`@move`/`@copy`), `@diffs` atomic batch with rollback, `@meta` blocks, `.avenpatch` serialization |
| **M6 — Stdlib** | ✅ Complete | `aven/std/io`, `fs`, `http`, `json`, `math`, `time`, `str`, `collections` — 8 modules, all as `NativeFn` closures |
| **M7 — Self-hosting prep** | ✅ Complete | Span-aware errors, `aven fmt`, `aven repl`, full spec coverage audit, non-trivial AVEN program, `@uncertain` deploy blocker, `@intent` index dump |

**All M1–M7 milestones are complete.** 503+ tests passing, zero warnings. The seed interpreter is stable.

**Stage 2 (self-hosted AVEN compiler written in AVEN) is complete.** All 8 pipeline components are implemented in `aven-core/`. The full `tokenize → parse → check → eval` pipeline is wired in `driver.aven` and runnable today on the seed interpreter. See [ROADMAP.md](ROADMAP.md) for the full Stage 2 milestone log.

See [ROADMAP.md](ROADMAP.md) for the full milestone log and [AVEN_SPEC.md](AVEN_SPEC.md) for the language specification.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).
