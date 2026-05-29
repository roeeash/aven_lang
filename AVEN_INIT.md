# AVEN — Design Notes

## The Problem with Existing Languages

Every popular language was designed for **humans typing on keyboards**. That means:
- Multiple ways to express the same thing (style wars)
- Ambiguous grammars requiring lookahead/context
- Visual structure that is fragile under programmatic editing (indentation, closing brackets)

An AI-native language should optimize for **structural predictability and zero ambiguity**. The benefit is not token count — it's that every expression has exactly one valid parse, which reduces hallucination-induced syntax errors in generated code.

---

## Design: `AVEN` — Agent Vector Expression Notation

**Core principles:**

| Principle | What it means |
|---|---|
| **Canonical form** | One and only one way to write anything |
| **Intent-first** | Every expression declares its purpose before its content |
| **Tree-native** | Structure is AST-level, not inferred from whitespace — indentation is display convention only |
| **Effect-typed** | Types encode what a function can *do* (err, IO, async), not just what it *is* |
| **Patch-friendly** | Any subtree can be replaced by selector path without affecting siblings |

---

### Syntax sketch

```aven
; Definitions use sigil-prefixed intent markers
@fn greet :: name:Str -> Str
  @ret "Hello, " + name

; Data is typed inline, no declaration ceremony
@let user :: {name:"Alice" age:30 role:#admin}

; Control flow is expression-only, no statements
@if user.age > 18
  @then @call greet user.name
  @else "too young"

; Effects are always explicit — no hidden IO
@io.write stdout "result: " + result

; Pattern matching is first-class
@match user.role
  #admin -> @call admin_panel
  #guest -> @call guest_view
  _      -> @err "unknown role"

; Async is structural, not syntactic sugar
@async
  @await fetch_data endpoint
  @await process_data _

; Modules import by capability, not filesystem path
@use [read write] from fs
@use [get post]   from http
```

---

### Why this is efficient for AI agents specifically

**Canonical vocabulary:**
- AVEN uses a fixed sigil set (`@fn`, `@let`, `@ret`, etc.) so an agent never has to choose between `function`, `func`, `def`, `fn`, or `fun`. Canonical form eliminates style variation, not token count.
- `::` is the universal type annotation separator — no context-sensitive colon/equals ambiguity
- Sigils (`@`, `#`, `_`) carry unambiguous syntactic roles per character

**Zero ambiguity:**
- Every expression starts with an `@intent` marker — the parser never needs lookahead
- `#symbol` is always an enum/tag, never a variable
- `_` is always wildcard/discard
- No operator precedence table — all binary ops require explicit grouping with `()`. This is a deliberate unambiguity choice with precedent in Smalltalk and Pony: more verbose expressions, but zero precedence-related bugs in generated code.

**AI-specific features:**
- `@intent "natural description"` — inline docstring that's part of the AST, not a comment
- `@uncertain` block — a **first-class AST annotation** meaning "this block needs review." Written explicitly (like a typed `TODO`), not inferred from model confidence. Tooling can lint, highlight, or block deployment of `@uncertain` nodes.
- `@diff` notation — AST-level semantic patches (selector-addressed, not line-addressed). See AVEN_SPEC.md §2 for the full format.
- `@ctx` — explicit context threading (no hidden global state ever)

---

### What I'd cut entirely

- String interpolation syntax (too many variants, just use `+`)
- Implicit returns
- Exceptions (only `@err` values, always explicit)
- Inheritance (composition only)
- Semicolons, commas as separators (whitespace-delimited, canonical)

---

The biggest tradeoff: this language is **hostile to humans** — the sigil density feels ugly, and there's no syntactic sugar for common patterns. That's fine, because the target consumer is an agent, not a text editor.

---

## The Bootstrapping Plan

### Stage 1: Seed interpreter — **Rust**

Write a minimal AVEN interpreter in Rust. Why Rust specifically:
- No GC (agents need predictable latency, no pause spikes)
- Compiles to WASM natively — agents run everywhere, including sandboxed
- `nom` / `pest` make writing zero-copy parsers fast
- Memory safety without a runtime overhead

This seed interpreter only needs to handle ~20% of AVEN's surface area — just enough to run the next stage. The seed is the reference implementation until the self-hosted compiler reaches parity.

### Stage 2: Self-host — **AVEN itself**

Once the seed is stable, write the full AVEN compiler *in AVEN* and use the Rust seed to compile it. Self-hosting is a long-term goal — Rust itself took years from its OCaml seed to a self-hosted compiler. The seed is not throwaway.

Self-hosting matters because:
- Forces the language to be expressive enough to write real software
- The compiler becomes a canonical example of idiomatic AVEN
- Agents can read and modify the compiler in the same language they're executing

### Stage 3: Two runtimes

| Runtime | Written in | Purpose |
|---|---|---|
| `aven-native` | AVEN → LLVM | Fast execution, production |
| `aven-llm` | — | Agent-driven interpreter for simulation and dry-run |

`aven-llm` is an interpreter mode where an agent reads AVEN code and produces a structured execution trace — useful for test simulation and reasoning about behavior before committing to a real run. It is not a production runtime and makes no determinism guarantees.

---

## The honest answer

**Rust for the seed, AVEN for everything after.** The first Rust compiler was written in OCaml; once Rust was strong enough, it was rewritten in itself. AVEN follows the same path.

The adoption path still requires a parser, runtime, and standard library. The advantage over human-targeted languages is that the tooling doesn't need IDE integration, autocomplete, or documentation UX — structured output from a spec is sufficient for agents to emit valid AVEN.
