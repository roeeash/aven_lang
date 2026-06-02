# AVEN Roadmap — Stage 2: Self-Hosting

This file is the complete, actionable plan for **Stage 2 — writing the AVEN compiler in AVEN**. Stage 1 (the Rust seed, M1–M7) is complete and its history lives in git; this roadmap is scoped to the self-hosting effort and contains every detail needed to execute it end to end.

> **The point of Stage 2.** The AVEN compiler is itself an AVEN program. The Rust seed (Stage 1) stops being *the language* and becomes the *bootstrap* — it exists only to run the AVEN-written compiler until that compiler can process its own source. Once the bootstrap fixpoint holds (S2.8), the seed is feature-frozen and all future language work happens in AVEN.

---

## Premise and current state

M1–M7 are complete: the Rust seed at `src/` (lexer, parser, typechecker, module resolver, `@diff` engine, 8 stdlib modules, `fmt`, `repl`, `verify`, `intent`) lexes, parses, type-checks, and evaluates the AVEN v0.1 subset, with **488 seed tests** passing (plus Stage-2 seed extensions added since: +7 for S2.0, +27 for S2.0b). The seed is a tree-walking **interpreter**, not a code generator — so the first self-hosted artifact is necessarily an **AVEN-written interpreter/checker** that the seed runs, not a native-code compiler. Native codegen is Stage 3a and is explicitly out of scope here.

The bootstrap target is the *compiler-as-AVEN-program*, run as:

```text
aven run aven-core/main.aven -- <target.aven>
```

The seed interprets the AVEN compiler, which in turn lexes/checks/evaluates the target. Self-hosting is achieved when the AVEN compiler can correctly process its own source (S2.8), with output parity against the seed.

### Stable build (for reproducibility)

```text
$ rustc --version
rustc 1.95.0 (59807616e 2026-04-14)
$ cargo --version
cargo 1.95.0 (f2d3ce0bd 2026-03-21)
```

Last full seed verification: 436 tests passing (`cargo build && cargo test`, 2026-05-25, zero warnings) on the M7 baseline. Stage-2 seed extensions (S2.0: +7, S2.0b: +27) were verified **source-level only** because `cargo`/`rustc` are unavailable in the current sandbox; `cargo build && cargo test` (488 + 7 + 27) is QUEUED for re-verification when a toolchain exists — it is not a per-stage gate while no toolchain is present.

---

## The dialect constraint (read first — this gates everything)

The seed implements a **restricted dialect** of the `AVEN_SPEC.md` surface, and the compiler-in-AVEN must be written in exactly what the seed accepts, not the idealized spec. The authoritative reference is **`aven-core/SUBSET.md`** (frozen in S2.0). Summary of the load-bearing constraints:

| Spec says | Seed actually accepts | Consequence for the AVEN compiler |
|---|---|---|
| Infix binary ops (`a + b`) | Parenthesized **Lisp-prefix** (`(+ a b)`); only `+ - * /` produce `Expr::Arithmetic` | Write all arithmetic/string ops in prefix form |
| `@match` over rich patterns | Symbol-tag `Tag(String)` + `TagBind(String,String)` single-var + wildcard `_` only | Encode AST dispatch as `#tag`-keyed unions, never literal/structural patterns |
| Lists `[T]`, maps, sets as native literals | No native literal syntax; sequences are `\n`-joined `Str` (AVEN level) or `Value::Map`/`Value::Set` (runtime) | AST node lists are `\n`-joined `Str`; this is the canonical encoding (locked S2.0) |
| Indentation-delimited blocks | Indent-tracked but newline-separated `Block(Vec<Expr>)`; returns last expr | Match the seed's real block grammar |
| Anonymous functions / closures | `@fn` **requires a name + `::`** — no anonymous-fn form | All iteration is via top-level named recursive `@fn`; no closure-based loops |
| Comparison / boolean ops | Added in **S2.0b** as prefix-call builtins (`int_lt`, `bool_and`, `str_ge`, …) — NOT operators, NOT tokens | Loop bounds / predicates use `(@call int_lt a b)`, `(@call str_ge c "0")`, etc. |

Where the compiler genuinely needs a feature the seed lacks, the fix is a **small, additive seed extension** — recorded as an `Sx.y-seed`-style sub-item — never a silent dependency on unimplemented spec behavior. S2.0b is the canonical example: the lexer needed comparison/boolean primitives, so they were added to the seed before S2.1 could proceed.

### Canonical AST/token encoding (locked in S2.0 — `aven-core/AST_ENCODING.md`)

- **Tokens**: one per line, `<kind>` or `<kind> <value>`. Examples: `LeftParen`; `Ident add`; `Integer 42`; `String "hello"`; `EffectArrow ?!~`; empty effect → `EffectArrow []`.
- **AST**: a parenthesized S-expression with `span` and `NodeId` **elided** (so dumps are diff-stable). Hand-formatted in the seed's dump path — **no `{:?}`/`{:#?}` anywhere** in the dump (raw Debug leaks `SourceSpan`/`NodeId` and is forbidden).
- These span-stable dumps are the **golden substrate** for every S2.1+ parity check.

---

## Strategy: vertical bootstrap, frontend-first

Build the compiler in the same dependency order the seed was built (M1→M7), each milestone validated for **output parity** against the corresponding seed stage before moving on. Frontend (lexer→parser→checker→resolver) lands before the backend, because the frontend can be checked against the seed's debug dumps (`aven emit-tokens` / `aven emit-ast`) without needing a working AVEN evaluator. Each milestone reuses the seed's existing `tests/integration.rs` corpus as golden fixtures: the AVEN component must produce results that match the seed's on the same inputs.

**Parity definition (the gate).** Parity = *identical verdict* (pass/fail) **+** *identical canonical AST/token dump* on the same input. Error *message text* parity is best-effort, **not** a gate.

---

## Milestone ladder (S2)

### S2.0 — Bootstrap prerequisites — ✅ Done

| Item | Notes |
|---|---|
| Re-verify seed build | `cargo build && cargo test` green before any Stage 2 work — blocks everything downstream (QUEUED: no toolchain in sandbox; M7 baseline 436 green) |
| Freeze the seed-AVEN subset reference | `aven-core/SUBSET.md` documents exactly what the interpreter accepts, confirmed against `parser.rs`/`eval.rs`; this is the language the compiler is written in |
| Seed debug dumps | `aven emit-tokens` / `aven emit-ast` subcommands print canonical, diffable output (S2.0a) |
| `aven-core/` layout | `aven-core/{lexer,parser,check,module,diff,eval,driver}.aven`, `aven-core/main.aven`, `aven-core/tests/`, fixtures mirrored from seed `tests/` |
| Golden-test harness | A script that runs both seed-native and AVEN-hosted paths over each fixture and diffs the outputs; parity = pass |
| AST-encoding decision | Tagged-`#kind` records with `node_id`/`span` fields, dumped as the span-elided S-expr in `AST_ENCODING.md`. Load-bearing for S2.1–S2.7 — locked here |
| Span-stable dump format | `emit-ast` redesigned away from raw `{:#?}` to a hand-formatted parenthesized S-expr (no `SourceSpan`/`NodeId` leakage) |
| Err-path + edge-input dump tests | `emit_ast_str` on parse-error input returns `Err`; `emit_tokens_str` on empty/whitespace returns `[Eof]` |

### S2.0a — `emit-tokens` / `emit-ast` canonical debug dumps — ✅ Done

First Stage-2 cycle. Added `emit-tokens` / `emit-ast` subcommands to the seed CLI (subcommand dispatch matching `verify`/`intent`, not `--flags`). `src/main.rs` gained `run_emit_tokens`/`run_emit_ast`; `src/lib.rs` exports `emit_tokens_str(&str) -> Vec<Token>`, `emit_ast_str(&str) -> Result<Expr, ParseError>`, re-exports `Token`. Six integration tests. These dumps are the parity substrate for the AVEN lexer/parser.

### S2.0b — seed comparison & boolean primitives — ✅ Done

Prerequisite stage inserted ahead of S2.1 after source-level verification of the first lexer attempt exposed a hard blocker: the seed-AVEN subset had NO comparison (`<`/`>`/`>=`/`<=`/int-`==`) or boolean (`and`/`or`/`not`) primitives — only `str_eq` — and those operator characters aren't tokens, so they can't be written. Without integer/char comparison and boolean combinators you cannot express loop bounds or branch predicates, so no compiler pass can be written in AVEN.

This stage adds **12 native-fn builtins** to `src/eval.rs`, each mirroring the `str_eq` template (arity guard → typed match → `Value::Bool`), invoked as `(@call name args…)` — no new tokens or operators:

- **Integer comparison** (`src/eval.rs:1951–2049`): `int_eq`, `int_lt`, `int_gt`, `int_le`, `int_ge` — two `Value::Int` → `Value::Bool`; non-Int → `EvalError::TypeError`.
- **Boolean logic** (`src/eval.rs:2051–2110`): `bool_and`/`bool_or` (arity 2), `bool_not` (arity 1, `Value::Bool(!a)`); non-Bool → `EvalError::TypeError`.
- **Lexicographic string/char ordering** (`src/eval.rs:2113–2191`): `str_lt`, `str_gt`, `str_le`, `str_ge` — Unicode-codepoint order on single-char `Str` from `str_get`, enabling digit/letter range tests; non-Str → `EvalError::TypeError`.

27 `test_builtin_*` integration tests cover true/false cases plus one type-error per category. Verified source-level (each builtin registered once via grep; operators traced against names — none flipped; added lines net-zero braces/parens via git-diff-scoped count). Documented in `SUBSET.md` §"Comparison & Boolean Primitives". Opus APPROVED Round 1. **Unblocks S2.1 and every downstream pass.**

### S2.1 — Lexer in AVEN — ▶ ACTIVE (unblocked by S2.0b)

| Item | Notes |
|---|---|
| `read source → token stream` | Input via `aven/std/fs` read; output the canonical token encoding from S2.0 |
| Token coverage | All 66 token kinds: sigil tokens, keywords, ints (incl. `1_000`), strings + escapes, `#symbol`, all 8 effect arrows, brackets, `::`, `/`, comments stripped |
| Parity test | AVEN lexer output over each fixture matches seed `emit-tokens` exactly |

Full implementation spec for S2.1 is in **§"Active stage spec: S2.1"** below.

### S2.2 — Parser in AVEN

| Item | Notes |
|---|---|
| Recursive-descent parser | One-token lookahead, prefix-op grammar; builds the tagged-record AST from S2.0 with monotonic `node_id`s. **No anonymous closures** — iteration via top-level named recursive `@fn` |
| Form coverage | `@fn`/`@let`/`@ret`/`@if`/`@match`/`@err`/`@ok`/blocks/records/`#symbol`/`@mod`/`@use`/`@pub`/`@intent`/`@uncertain`/`@ctx`/`@diff` |
| AST serializer | Canonical AST→`Str` printer so output is diffable against seed `emit-ast` |
| Parity test | Parse every fixture; serialized AST matches the seed's |

---

## Active stage spec: S2.2 — Parser in AVEN

**Goal.** Write an AVEN-language recursive-descent parser in `aven-core/parser.aven` that reads a token stream (from S2.1 lexer) and outputs a canonical AST as a `\n`-joined `Str` (per `AST_ENCODING.md`), with output matching the seed's `emit-ast` dump format exactly. The parser must recognize all AVEN expression forms within the frozen seed-AVEN subset and thread parser state (position, token stream, monotonic node counter) through all function returns — no mutable state, no closures.

**Unblocked by S2.1.** The S2.1 lexer produces a `\n`-joined token stream in canonical format. The S2.0c seed supports tuple literals and destructuring, enabling multi-value returns for state threading. The S2.0b comparison/boolean primitives enable loop bounds and branch predicates for recursive descent.

**Files touched.**
- Primary: `aven-core/parser.aven` — the implementation (new file).
- Test fixtures: `aven-core/tests/parser-fixtures/` (new) — golden `.aven` source files + expected `.ast` dump files (seed's `emit-ast` output for each).

**Parser architecture.**

Parser state is represented as a `(Int, Str)` tuple: position within the token stream (`Int`) and the token stream itself (`Str`, `\n`-joined). All parser functions return `(pos', ast)` tuples, threading state forward.

- **Main entry point:** `@fn parse :: tokens:Str -> Str` — reads tokens, returns canonical AST (or error marker).
- **State threading:** Each recursive function has signature `@fn name :: tokens:Str pos:Int -> (Int, Expr)` and returns `(next_pos, expr)`.
- **Token consumption:** Use `str_get(tokens, eol_idx + 1)` to extract current line; scan for `\n` to find next line offset; advance position by line length.
- **One-token lookahead:** Inspect current token before consuming it; handle prefix operators and keyword dispatch.
- **Node ID allocation:** Global counter threaded as `Int` through all parse functions; incremented on each AST node creation.

**Form priority (implement in order).**

1. `Lit` (Int, Float, Str, Bool, Nil) — `(Int 42)`, `(Float 3.14)`, `(Str "...")`, `(Bool @true)`, `(Nil)`
2. `Var` + `Symbol` — variable names, `#tag` symbols
3. `Let` — `(Let name expr)`
4. `FnDef` — `(FnDef name (params) body return-type effects [caps])`
5. `FnCall` — `(FnCall name arg1 arg2 ...)`
6. `Arithmetic` — `(Arithmetic Add left right)` from `(+ a b)` prefix syntax
7. `If` — `(If cond then-branch else-branch)`
8. `Ret` — `(Ret expr)`
9. `Block` — sequence of expressions, `\n`-joined in `Str` form
10. `Match` — `(Match scrutinee patterns-str)` with patterns as `TagBind tag var -> expr`
11. `Tagged` — `(@ok v)` → `(Tagged ok (Just v))`
12. `@uncertain`, `@intent` wrappers
13. `Mod`, `Use`, `Pub` — module/use/pub declarations

**Token parsing helpers.**

- `@fn current-token :: tokens:Str pos:Int -> Str` — extract token at position (scan to `\n`; return line substring).
- `@fn next-pos :: tokens:Str pos:Int -> Int` — find next position after current token (position of char after `\n`).
- `@fn consume :: tokens:Str pos:Int -> (Int, Str)` — return current token and next position.
- `@fn expect :: tokens:Str pos:Int kind:Str -> (Int, ())` — assert current token matches kind; error if not.

**AST encoding output (per `AST_ENCODING.md`).**

- Literals: `(Int 42)`, `(Float 3.14)`, `(Str "content")`, `(Bool true)`, `(Nil)`
- Variables: `(Var x)`
- Symbols: `(Symbol ok)`
- FnDef: `(FnDef add ((a Int) (b Int)) (Ret (Arithmetic Add (Var a) (Var b))) Int [])`
- FnCall: `(FnCall add (Int 1) (Int 2))`
- If: `(If (Bool true) (Int 100) (Int 0))`
- Arithmetic: `(Arithmetic Add (Int 1) (Int 2))`
- Block: `(Block expr1\nexpr2\nexpr3)` — subexpressions as `\n`-joined `Str`
- Match: `(Match scrutinee::(Var x) patterns::(TagBind ok v -> (Var v))\n(Wildcard -> (Int 0)))`

**Test fixtures (source-level; no runtime harness yet).**

- `literals.aven` → `literals.ast`: ints, floats, strings, bools, nil
- `variables.aven` → `variables.ast`: var names, symbol tags
- `let.aven` → `let.ast`: let bindings, nested lets
- `fn-def.aven` → `fn-def.ast`: function definitions with type annotations, effects
- `fn-call.aven` → `fn-call.ast`: function calls with multiple args
- `arithmetic.aven` → `arithmetic.ast`: prefix `(+ a b)`, `(* (+ 1 2) 3)`, etc.
- `if-then-else.aven` → `if-then-else.ast`: conditional branches, nested ifs
- `match-patterns.aven` → `match-patterns.ast`: `@match` with `#tag` dispatch, `TagBind`, wildcard

Hand-trace: for each fixture, seed-parse via `aven emit-ast <fixture> > seed.ast`, then verify the AVEN parser output matches line-for-line.

**Definition of done (S2.2).**
- `parser.aven` complete, conforms to `SUBSET.md` (no closures, prefix-only calls, state threading via tuples).
- All 10 priority forms recognized and emitted in canonical AST format.
- AST serializer produces span-elided S-expressions per `AST_ENCODING.md`.
- All 8 test fixtures exist; `.ast` files prepared by hand-tracing against seed `emit-ast`.
- Tuple state threading is idiomatic (no `|>`, no mutable records).
- **Queued for runtime verification** (when `aven` binary exists): `parse(tokenize(source))` matches seed `emit-ast` over the `tests/integration.rs` corpus.

**Out of scope.** Type checker, module resolver, evaluator, `@diff` engine; error recovery (parser emits best-effort AST); actual execution in sandbox.

### S2.3 — Type & effect checker in AVEN

| Item | Notes |
|---|---|
| Bidirectional checker | Port the seed's `typechecker.rs` logic: synth/check, `types_compatible`, compound returns, `FnCall` arity + arg types |
| Effect subset rule | `EffectSet` as an AVEN record of `?!~` flags; callee effects ⊆ caller effects; `@cap`↔`!` tie |
| `@uncertain` boundary | Reject uncertain values crossing typed boundaries unless acknowledged (the five seed boundary sites: `Var` lookup, `Let`, `FnDef` return, `Block` let, `Block` final) |
| Parity test | Every seed typechecker test reproduced; pass/fail verdict and message-shape parity |

### S2.4 — Module resolver in AVEN

| Item | Notes |
|---|---|
| Identity + caps map | `@mod` dotted identity, `@pub` export sets, `@use` capability-subset verification |
| DAG + topo | Cycle detection (DFS, self-loops excluded), topological sort (Kahn), topo-ordered checking |
| `@ctx` threading | `@ctx.get ctx key → ?T`, threaded via args, never global |
| Parity test | Reproduce the §3.9 7-step narrative test end-to-end |

### S2.5 — `@diff` engine in AVEN

| Item | Notes |
|---|---|
| Selector parser | `/fn greet/body/ret`, positional `[n]`, entity-name resolution (scan `Block` children for `FnDef`/`Let`/`Mod` by `.name`) |
| Apply + ops | `@replace`/`@insert`/`@delete`/`@move`/`@copy`; `@diffs` atomic batch with clone-and-swap rollback; post-patch re-typecheck (reject type-invalid results) |
| `.avenpatch` | `@patch-for` round-trip serialize/parse |
| Parity test | Reproduce the seed M5 diff suite, including rollback-on-typecheck-failure |

### S2.6 — Evaluator (backend) in AVEN

| Item | Notes |
|---|---|
| Tree-walking interpreter | `eval(node, env)`; scoped env with parent chain; closure capture; `@ret`/`@match`/`@err`-as-value semantics |
| Stdlib bridge | The AVEN evaluator delegates IO/fs/json/etc. by `@use`-ing the seed stdlib (no FFI; the seed already exposes these as `NativeFn`) — document the bridge boundary |
| Parity test | Run the M7 JSON-parser AVEN program through the AVEN-hosted evaluator; output equals the seed's |

### S2.7 — Driver + CLI in AVEN

| Item | Notes |
|---|---|
| Pipeline wiring | `lex → parse → resolve → check → (diff) → eval` orchestrated in `aven-core/main.aven` |
| Subcommands | `run`, `check`, `fmt`, `patch`, `intent` implemented in AVEN |
| `fmt` parity | AVEN re-printer is idempotent and byte-identical to seed `aven fmt` on the fixture corpus |

### S2.8 — The self-hosting fixpoint (bootstrap closure)

The milestone that defines "self-hosted." Let `C0` = the Rust seed.

| Step | Check |
|---|---|
| Stage A | `C0` runs the AVEN compiler (`C1 = C0(aven-core)`); `C1` `check`s + `fmt`s the entire fixture corpus with full parity to `C0` |
| Stage B | `C1` is run over **its own source** (`aven-core/*.aven`): it lexes, parses, type-checks, and `fmt`s the compiler itself without error |
| Fixpoint | `fmt` of the compiler source is idempotent under `C1`; `C1`'s check/eval verdicts on the corpus are identical to `C0`'s. The AVEN compiler successfully processes the AVEN compiler |
| Freeze | Once the fixpoint holds, the Rust seed is feature-frozen — no new language features land in Rust; all subsequent work is in AVEN |

**Limit of interpreter-based self-hosting.** Because `C0` is an interpreter, `C1` is "compiled" in the sense of *type-checked and accepted/evaluated*, not lowered to a standalone binary. True binary self-hosting (a compiler that emits its own executable) arrives only with Stage 3a codegen. S2.8 is the legitimate self-hosting milestone for an interpreted seed: the language can fully process its own implementation.

### S2.9 — Repo split & freeze

| Item | Notes |
|---|---|
| Monorepo split | Per project instructions, split into `aven-spec` / `aven-seed` / `aven-core` now that the seed compiles real programs |
| Freeze + tag | Tag the frozen seed; `aven-core` becomes the active development surface |
| Stage 3 pointers | `aven-native` (AVEN→LLVM) and `aven-llm` (dry-run agent interpreter) become the next horizon |

---

## Active stage spec: S2.1 — Lexer in AVEN

**Goal.** Write an AVEN-language lexer in `aven-core/lexer.aven` that reads source text and produces a canonical token stream (via `aven/std/fs` file input, `\n`-joined `Str` output) matching the seed's `emit-tokens` dump format exactly. The lexer must recognize all 66 token kinds from `src/lexer.rs` and handle comments, escapes, negative numbers, symbol names, and all 8 effect arrows.

**Now unblocked.** The S2.0b primitives (`int_lt`/`int_eq`/`str_lt`/`str_ge`/`bool_and`/`bool_not`/…) supply the comparison and boolean combinators the lexer needs for loop bounds and range tests. The first lexer attempt (record-literal/closure-based, non-runnable) was reverted to a stub; the golden `.tokens` fixtures remain valid targets. Re-plan and implement against the real builtins.

**Files touched.**
- Primary: `aven-core/lexer.aven` — the implementation (replacing the stub).
- Test fixtures: `aven-core/tests/lexer-fixtures/` (new) — golden `.aven` source files + expected `.tokens` dump files.

**Specific changes — in `lexer.aven`:**
- Represent the `Lexer` as threaded state (position, input char access, lookahead) through function returns — seed-AVEN has **no mutable structs and no closures**, so all iteration is **top-level named recursive `@fn`**.
- `@fn next-token :: lex -> token` — returns a single token, advances position.
- `@fn tokenize :: source:Str -> tokens:Str` — calls `next-token` repeatedly, accumulating `\n`-joined token strings until `Eof`; skip `Newline` tokens (per `lexer.rs:477`).
- Helpers: `read-number` (integers, floats, exponents, underscores), `read-string` (escapes `\n`, `\t`, `\\`, `\"`), `read-ident` (alphanumeric, dash, dot), `read-symbol-name` (alphanumeric, underscore only), `skip-whitespace`, `skip-comment`.
- Use real builtins only: `str_get`/`str_len`/`str_sub`/`str_eq`, plus S2.0b `(@call int_lt a b)`, and **inclusive** digit ranges via `(@call bool_and (@call str_ge c "0") (@call str_le c "9"))`. (Note: the inclusive upper bound is `str_le c "9"`, not `str_lt c "9"`.)

**Token emission — canonical dump format per `AST_ENCODING.md`:**
- No-data tokens: bare names (`At`, `Let`, `Fn`, …, `Eof`) — 46 variants.
- `EffectArrow(EffectSet)`: `EffectArrow <sigils>` where `<sigils>` is empty `[]` or a sequence of `?`, `!`, `~` (e.g., `EffectArrow ?!~`).
- Data-bearing literals: `Ident <name>`, `Integer <value>`, `Float <value>`, `String "<quoted>"`.

**Core tokenization logic (seed reference: `src/lexer.rs:286–457`):**
- Skip whitespace (space, tab, `\r`) and comments (`;` to EOL).
- On `@`: read ident, match keyword table (47 entries, lines 304–345) → keyword token or `Ident @name`.
- On `#`: read symbol name (alphanumeric/underscore only) → `Ident #name`.
- On `"`: `read-string` → `String`.
- On `:`: lookahead for `::` → `DoubleColon` else `Colon`.
- On `-`: lookahead `>` → `Arrow`; else `?`/`!`/`~` sequence ending in `>` → `EffectArrow`; else digit → negative number; else `Minus`.
- On `+`, `*`, `/`, `?`, etc.: single-char tokens.
- On `(`, `)`, `[`, `]`, `{`, `}`, `|`, `,`, `_`: single-char tokens.
- On digit: `read-number` → `Integer`/`Float`.
- On letter: `read-ident` → `Ident`.
- On newline: `Newline` (filtered by `tokenize`).
- On EOF: `Eof`.

**Test fixtures (no executable harness — source-level + hand-trace verification):**
- `keywords.aven` → `keywords.tokens`: all 47 keywords.
- `integers.aven` → `integers.tokens`: `42`, `-7`, `1_000`.
- `floats.aven` → `floats.tokens`: `3.14`, `-0.001`, `1.5e2`, `1e-3`.
- `strings.aven` → `strings.tokens`: simple strings + escapes (`\"`, `\\`, `\n`, `\t`).
- `symbols.aven` → `symbols.tokens`: `#ok`, `#error`, `#admin` → `Ident #ok`, etc.
- `effect-arrows.aven` → `effect-arrows.tokens`: all 8 arrows (`->`, `-?>`, `-!>`, `-~>`, `-?!>`, `-?~>`, `-!~>`, `-?!~>`).
- `comments.aven` → `comments.tokens`: comments stripped.
- `mixed.aven` → `mixed.tokens`: a small real program (e.g., `@fn add a b -> (+ a b)`).
- Hand-trace: for each fixture, `aven emit-tokens <fixture> > seed.tokens`, then verify the AVEN lexer output `Str` matches line-for-line.

**Definition of done (S2.1).**
- `lexer.aven` complete, conforms to `SUBSET.md` (prefix-only calls, `@fn`/`@match` `#tag` dispatch, `\n`-joined Str output, no closures).
- All 66 token kinds have an emit branch.
- Dump format matches `AST_ENCODING.md` exactly: one token per line, `<kind>` or `<kind> <value>`.
- All fixtures exist; `.tokens` files prepared by hand-tracing against seed `emit-tokens`.
- Comments stripped; negative numbers, escaped strings, symbol names, all 8 effect arrows recognized.
- **Queued for runtime verification** (when a compiled `aven` binary exists): `tokenize(source)` matches seed `emit-tokens` over the `tests/integration.rs` corpus via the golden harness.

**Out of scope.** Parser/checker/evaluator/resolver/`@diff`; actual execution in the sandbox (no `cargo`/`aven` binary); error recovery (the seed lexer emits no errors — malformed input produces best-effort tokens).

---

## Likely seed extensions (additive only)

Kept minimal so the frozen seed stays small. `--emit-tokens`/`--emit-ast` dumps (S2.0a) and comparison/boolean/string-ordering primitives (S2.0b) are already landed. Anticipated remaining:

- Recursion-depth / stack headroom sufficient to interpret a recursive-descent parser written in AVEN (S2.2 risk).
- Any string primitive the parser needs that `aven/std/str` lacks (e.g., slice-by-range) — verify against `str` before adding.
- An AST (de)serialization helper if the chosen AVEN encoding can't round-trip through existing forms.

---

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| **Dialect drift** — writing the compiler against spec-AVEN instead of seed-AVEN | `SUBSET.md` is mandatory; lint `aven-core/*.aven` with the seed before every commit |
| **AST-encoding churn** forces rewrites across S2.1–S2.7 | Encoding locked in S2.0 (`AST_ENCODING.md`); prototype against one fixture before building S2.1 |
| **Interpreting an interpreter is slow** — `C0(C1(target))` is two interpreter layers | Acceptable for bootstrap; performance is a Stage 3a concern, not a Stage 2 gate |
| **Recursion/stack limits** when parsing deep ASTs | Surface early via S2.2 deep-nesting fixtures; raise seed limit as an additive extension if hit |
| **No closures in seed-AVEN** | All iteration via top-level named recursive `@fn`; restructure any loop that wants a closure |
| **Parity ambiguity** for error messages | Parity = identical verdict + identical canonical dump; message text is best-effort, not a gate |
| **Scope creep into codegen** | Stage 2 ends at the interpreter fixpoint (S2.8); native compilation is firewalled into Stage 3a |

---

## Workflow (orchestrated 3-agent cycle + Opus gate)

Each S2.x milestone runs the established loop: **Planner → Programmer → Reviewer → fix** (3-agent Haiku cycle) with an **Opus quality gate** (≤3 rounds) closing the milestone. The orchestrator independently verifies every agent structural claim (never trust — the first-lexer disaster and brace false-alarms were both caught only by independent inspection), filters reviewer false positives, and fixes trivial defects directly with documentation.

A milestone is **Done** only when: its parity tests pass (or are explicitly QUEUED with a documented reason when no toolchain exists), the seed build is green (or QUEUED), and Opus approves. Record outcomes and workflow notes in this file exactly as the S2.0a/S2.0/S2.0b entries above do. Opus rounds are logged verbatim to `workflow/opus-review-log.md`.

Note: in the current sandbox `cargo`/`rustc` are unavailable and the network is locked, so all verification is **source-level** (string-aware brace/paren balance, grep for symbols/tests, hand-tracing). `cargo build && cargo test` re-verification is queued for when a toolchain exists and is not a per-stage gate meanwhile.

---

### S2.1 — Lexer in AVEN — **Done**

**Outcome.** `aven-core/lexer.aven` implements a complete AVEN-language lexer in the frozen seed-AVEN subset. State is threaded via `(Int, Str)` tuple returns using S2.0c syntax. All token kinds handled via recursive `@fn` dispatch with no closures, no mutable state, no `|>`. Key fixes required during review: tuple destructuring replacing `|>` and bare `@[n]` indexing; consistent `read-ident-loop` return type (raw ident, no prefix); negative floats dispatched to `read-float`/`read-exponent`; underscore stripping in numeric literals; underscore accepted as ident continuation. All 8 golden fixtures in `aven-core/tests/lexer-fixtures/`. SUBSET.md updated with S2.0b builtin summary. Opus approved Round 2. Runtime parity verification (against seed `emit-tokens`) QUEUED when `aven` binary available.

### S2.0c — Seed tuple + string builtins — **Done**

**Outcome.** `Expr::Tuple(Vec<Expr>, NodeId, SourceSpan)` and `Expr::TupleIndex { tuple, index, node_id, span }` added to AST. `Value::Tuple(Vec<Value>)` and `EvalError::IndexOutOfBounds { index, length }` added to eval. Parser recognizes `(a b)` and `(a, b)` tuple literals, `@[n]` postfix indexing, and `@let (a, b) :: expr` destructuring (desugared to Block+Let+TupleIndex). `str_eq` confirmed present. 15 new integration tests. 503 integration tests passing, zero warnings. Opus approved Round 2 after fixing: trailing-comma `)` consumption, empty-destructure error, propagating inner parse errors instead of swallowing, removing `Token::At` from `is_expression_start`. S2.1 lexer is now unblocked.

**Goal.** Add tuple literal syntax `(a, b)` and tuple destructuring `@let (x, y) :: expr` plus `@[0]`/`@[1]` indexing to the Rust seed, unblocking the AVEN lexer (S2.1) which requires multi-value returns threaded through computation. Also verify `str_eq` builtin exists and is accessible to AVEN.

**Files touched.**
- `/Users/roee.ashkenazi/Desktop/aven-lang/src/ast.rs` — add `Tuple(Vec<Expr>)` expr variant (before `Nil`).
- `/Users/roee.ashkenazi/Desktop/aven-lang/src/parser.rs` — tuple literal parsing + destructuring pattern recognition.
- `/Users/roee.ashkenazi/Desktop/aven-lang/src/eval.rs` — tuple eval + indexed access (`@[0]`/`@[1]`); verify `str_eq` registered.
- `/Users/roee.ashkenazi/Desktop/aven-lang/src/main.rs` — seed tests for tuple ops.

**Specific changes.**
- **ast.rs**: Insert `Tuple(Vec<Expr>, NodeId, SourceSpan)` before `Nil`; update `Pattern` enum to add `TupleDestructure(Vec<String>)` for `(a, b)` binding.
- **parser.rs**: Detect `(` followed by `,` → parse tuple literal; destructure in `@let (x, y) :: expr` via new pattern variant; parse `@[0]`/`@[1]` as indexed access (check if `LeftBracket` is a suffix token).
- **eval.rs**: `Tuple(Vec<Value>)` at runtime; index via `@[n]` token sequence (e.g., `expr @[0]` → get field 0); verify `str_eq` at lines ~1500–1600 (add if missing).
- **Integration tests** (8–10): tuple literals `(1 2)`, destructure `@let (a b) :: (1 2)` binds a=1 b=2, index `@[0]`, `@[1]`, nesting `((1 2) (3 4))`, string concat `(+ a b)` on strings.

**Definition of done.**
- `Tuple` expr + `TupleDestructure` pattern variant added + compiled error-free.
- Parser recognizes `(a, b)` tuples (comma-separated, no parens-only groups).
- Destructuring `@let (x, y) :: …` binds both names.
- Index syntax `@[0]`/`@[1]` extracts tuple fields; out of bounds → `EvalError::IndexOutOfBounds`.
- `str_eq` confirmed registered in `eval.rs` or added; 3 tests verify.
- All 488 existing seed tests still pass; 8 new tuple tests pass.

**Out of scope.** `|>` pipe operator (not needed for lexer; defer to later); recursive tuple nesting limit (accept Rust recursion defaults for now).

---

**Goal.** Implement a complete AVEN-language lexer in `aven-core/lexer.aven` that tokenizes source text into a canonical `\n`-joined stream matching the seed's `emit-tokens` output. The lexer must recognize all 66 token kinds, handle all escape sequences, symbol names, and effect arrows, and run under the frozen seed-AVEN subset (no closures, no mutable state, prefix-only calls).

**Files touched.**
- `/Users/roee.ashkenazi/Desktop/aven-lang/aven-core/lexer.aven` — lexer implementation (new file).
- `/Users/roee.ashkenazi/Desktop/aven-lang/aven-core/tests/lexer-fixtures/*.aven` — 8 test source files (new).
- `/Users/roee.ashkenazi/Desktop/aven-lang/aven-core/tests/lexer-fixtures/*.tokens` — 8 golden token dumps (new).

**Specific changes.**
- Implement `@fn tokenize :: source:Str -> Str` as the main entry point: reads source, returns `\n`-joined token stream with `Newline` tokens filtered out.
- Implement `@fn next-token :: state:Lexer -> (token:Token, state:Lexer)` as a helper: returns one token and new state (threaded position, input chars).
- Implement helpers: `read-number`, `read-string`, `read-ident`, `read-symbol-name`, `skip-whitespace`, `skip-comment` — all as top-level named `@fn` with state threading.
- Use only S2.0b builtins for comparisons: `int_lt`, `int_eq`, `str_lt`, `str_ge`, `bool_and`, `bool_not`, plus `str_get`, `str_len`, `str_eq` from stdlib.
- Emit tokens in canonical format per `AST_ENCODING.md`: bare token names for no-data variants (`At`, `Let`, …, `Eof`), and `<kind> <value>` for data-bearing ones (e.g., `Ident add`, `Integer 42`, `String "hello"`, `EffectArrow ?!~`).

**Tests to add** (source-level; no runtime harness yet).
- `keywords.aven` → `keywords.tokens`: all 47 `@` keywords tokenized correctly.
- `integers.aven` → `integers.tokens`: `42`, `-7`, `1_000`, `0` all emit `Integer <value>`.
- `floats.aven` → `floats.tokens`: `3.14`, `-0.001`, `1.5e2`, `1e-3` all emit `Float <value>`.
- `strings.aven` → `strings.tokens`: escapes (`\n`, `\t`, `\\`, `\"`) handled; output `String "<content>"`.
- `symbols.aven` → `symbols.tokens`: `#ok`, `#error`, `#admin` emit `Ident #ok` etc.
- `effect-arrows.aven` → `effect-arrows.tokens`: all 8 arrows (`->`, `-?>`, `-!>`, `-~>`, `-?!>`, `-?~>`, `-!~>`, `-?!~>`) tokenized.
- `comments.aven` → `comments.tokens`: comments (`;` to EOL) stripped; only code tokens appear.
- `mixed.aven` → `mixed.tokens`: real program excerpt (e.g., `@fn add a b -> (+ a b)`) lexed end-to-end.

**Definition of done.**
- `lexer.aven` complete, compiles/parses under seed interpreter without error.
- All 66 token kinds have an emit code path (reviewed by line count and function branches).
- Dump format is exact: one token per line, `<kind>` or `<kind> <value>`, no extra whitespace.
- All 8 test fixtures created; `.tokens` files hand-traced against seed `aven emit-tokens <fixture>`.
- Comments stripped; negative numbers, escaped strings, symbol names, all 8 effect arrows verified correct.

**Out of scope.** Parser (S2.2); checker/resolver/evaluator (S2.3–S2.6); runtime execution in sandbox (queued when `aven` binary exists); error recovery (seed lexer emits no errors, best-effort on malformed input).

---

## Definition of done (Stage 2)

- `aven-core/` contains a complete AVEN-written lexer, parser, checker, resolver, `@diff` engine, evaluator, and driver, all in the frozen seed-AVEN subset.
- Every seed `tests/integration.rs` fixture passes with parity through the AVEN-hosted pipeline.
- The S2.8 fixpoint holds: the AVEN compiler processes its own source with idempotent `fmt` and full corpus parity.
- The repo is split (S2.9) and the Rust seed is feature-frozen.

---

## Status snapshot

| Stage | State |
|---|---|
| S2.0a — emit-tokens/emit-ast dumps | ✅ Done |
| S2.0 — bootstrap prerequisites | ✅ Done |
| S2.0b — seed comparison & boolean primitives | ✅ Done |
| **S2.1 — Lexer in AVEN** | ✅ Done |
| S2.2 — Parser in AVEN | ✅ Done |
| S2.3 — Type & effect checker | ✅ Done |
| S2.4 — Module resolver | ✅ Done |
| S2.5 — `@diff` engine | ✅ Done |
| S2.6 — Evaluator (backend) | ✅ Done |
| S2.7 — Driver + CLI | Pending |
| S2.8 — Self-hosting fixpoint | Pending |
| S2.9 — Repo split & freeze | Pending |

---

### S2.2 — Parser in AVEN — **Done**

**Outcome.** `aven-core/parser.aven` implements a recursive-descent parser consuming `\n`-joined token stream → canonical AST per `AST_ENCODING.md`. State threaded as `remaining:Str` through all 21 functions. Covers: literals, `Var`, `Let`, `FnDef`, `Ret`, `If`, arithmetic (`+-*/`), `FnCall`. Key fix: all string concat sites used n-ary `(+ a b c...)` — rewritten to nested binary form since seed `+` is strictly 2-operand. `parse-paren` fallthrough fixed to advance. 6 golden fixtures in `aven-core/tests/parser-fixtures/`. Opus approved Round 2. Runtime parity QUEUED when `aven` binary available.

---

## Active Stage spec (archived): S2.2 — Parser in AVEN

**Goal.** Implement a recursive-descent parser in `aven-core/parser.aven` that consumes the S2.1 lexer's `\n`-joined token stream and produces a canonical `\n`-joined AST dump (per `AST_ENCODING.md`). The parser must run entirely within the frozen seed-AVEN subset: all state is threaded as a `remaining:Str` (the unparsed suffix of the token stream), no closures, all dispatch via top-level named recursive `@fn`.

**Files touched.**
- `aven-core/parser.aven` — new file, the implementation.
- `aven-core/tests/parser-fixtures/*.aven` — 6 source files (new).
- `aven-core/tests/parser-fixtures/*.ast` — 6 golden AST dumps (new).

**Token stream representation.** Each token is one line. The parser threads `remaining:Str` — the unparsed suffix of the token stream. Two primitives (implemented as helpers):
- `peek :: remaining:Str -> Str` — returns the text before the first `\n` (the current token kind+value).
- `advance :: remaining:Str -> Str` — returns everything after the first `\n` (drops current token).
- When `remaining` has no `\n`, `peek` returns `remaining` and `advance` returns `""`.

**Specific changes — `parser.aven`:**
- `@fn parse :: tokens:Str -> Str` — entry point: calls `parse-expr` then returns AST string.
- `@fn parse-expr :: remaining:Str -> (Str, Str)` — dispatches on `(peek remaining)`:
  - `"Let"` → `parse-let`
  - `"Fn"` → `parse-fn`
  - `"Ret"` → `parse-ret`
  - `"If"` → `parse-if`
  - `"Call"` → `parse-call` (inside `(@call fn args...)`)
  - `"LeftParen"` → `parse-paren` (arithmetic op or tuple)
  - `"Integer ..."` / `"Float ..."` / `"String ..."` / `"True"` / `"False"` → literal
  - `"Ident ..."` → `Var <name>` or `#symbol`
  - `"Match"` → `parse-match`
  - `"Ret"` → `parse-ret`
- Each `parse-X :: remaining:Str -> (ast:Str, remaining:Str)` returns (AST string, remaining tokens).
- Token-kind matching: `str_sub` on the `peek` result to extract the kind prefix (compare against `"Integer"`, `"Ident"`, etc.).
- `@fn parse-block :: remaining:Str -> (Str, Str)` — parses until `Eof` or unrecognized token, joins sub-ASTs with `\n`.

**AST output format** (per `AST_ENCODING.md`):
- Literals: `(Int 42)`, `(Float 3.14)`, `(Str "hello")`, `(Bool true)`.
- Var: `(Var name)`. Symbol: `(Symbol tagname)`.
- Let: `(Let name <expr>)`. Ret: `(Ret <expr>)`.
- FnDef: `(FnDef name (<params>) <body> <return-type> <effects>)`.
- If: `(If <cond> <then> <else>)`. Arithmetic: `(Arithmetic Add <l> <r>)`.
- FnCall: `(FnCall name <arg1> <arg2>...)`.

**Tests to add** (source-level, hand-traced against `aven emit-ast`):
- `let.aven` + `let.ast`: `@let x :: 42` → `(Let x (Int 42))`.
- `fn.aven` + `fn.ast`: `@fn add :: a:Int b:Int -> Int @ret (+ a b)` → full FnDef.
- `if.aven` + `if.ast`: `@if @true @then 1 @else 0` → `(If (Bool true) (Int 1) (Int 0))`.
- `call.aven` + `call.ast`: `(@call add 1 2)` → `(FnCall add (Int 1) (Int 2))`.
- `arith.aven` + `arith.ast`: `(+ (* 2 3) 4)` → nested `Arithmetic`.
- `block.aven` + `block.ast`: multi-statement block.

**Definition of done.**
- `parser.aven` parses under seed interpreter without error (source-level verified).
- All 6 fixtures exist; `.ast` files hand-traced against seed `aven emit-ast`.
- `parse-expr` covers: literals, `Var`, `Let`, `FnDef`, `Ret`, `If`, arithmetic, `FnCall`.
- State threading: every `parse-X` returns `(ast, remaining)` — no global state.
- No closures; all dispatch via named `@fn`.

**Out of scope.** `@match`, `@err`/`@ok`, `@mod`/`@use`/`@pub`, `@diff`, `@intent`, `@uncertain`, `@ctx` (defer to later parse passes); error recovery (best-effort); runtime parity test (QUEUED when binary available).

---

### S2.3 — Type checker in AVEN — **Done**

**Outcome.** `aven-core/check.aven` implements a structural type checker (23 functions) consuming `\n`-joined AST strings. Environment threaded as `\n`-joined `name:Type\n` entries, prepended for correct shadow semantics. Infers types for: Int/Float/Str/Bool literals, Var lookup, Let binding, Ret, If (cond must be Bool or Unknown), Arithmetic (operand type compatibility). Key Opus fixes: env-extend must prepend (not append) so newer bindings shadow older; if-cond check must allow Unknown via bool_or. 6 golden fixtures. Opus approved Round 2. Runtime parity QUEUED.

## Active Stage spec (archived): S2.3 — Type checker in AVEN

**Goal.** Implement a structural type checker in `aven-core/check.aven` that walks the canonical AST string (S2.2 parser output, `\n`-joined S-expressions) and returns `"PASS"` or `"ERROR: <msg>"`. Scope is a forward-only checker: infer types for literals, arithmetic, `Let` bindings, `Var` lookup, `If`, `Ret`, and `FnCall` arity. Full bidirectional inference and effect tracking are deferred; this stage establishes the environment-threading pattern and parity baseline.

**Files touched.**
- `aven-core/check.aven` — checker implementation (replaces stub).
- `aven-core/tests/check-fixtures/*.ast` — 6 AST input files (new).
- `aven-core/tests/check-fixtures/*.verdict` — 6 expected verdict files (new).

**Type environment representation.** A `\n`-joined string of `name:Type` entries (e.g. `"x:Int\ny:Str"`). Helpers: `env-lookup :: env:Str name:Str -> Str` (returns type or `"Unknown"`), `env-extend :: env:Str name:Str ty:Str -> Str` (appends `name:Type\n`).

**Token / AST node format.** The checker reads AST nodes by scanning the `\n`-joined AST string line by line (same `peek`/`advance` helpers as S2.2 parser, reused here). Each AST node is one line in the format from `AST_ENCODING.md`.

**Specific changes — `check.aven`:**
- `@fn check :: ast:Str -> Str` — entry: calls `check-block ast "" "Unknown"`, returns verdict.
- `@fn check-block :: ast:Str env:Str last-type:Str -> Str` — processes lines until `""`, returns `"PASS"` or `"ERROR: ..."`.
- `@fn check-node :: line:Str env:Str -> (Str, Str)` — returns `(type, updated-env)` for one AST line. Dispatch on prefix:
  - `"(Int "` → type `"Int"`, env unchanged.
  - `"(Float "` → type `"Float"`, env unchanged.
  - `"(Str "` → type `"Str"`, env unchanged.
  - `"(Bool "` → type `"Bool"`, env unchanged.
  - `"(Var "` → look up name in env; return its type or `"Unknown"`.
  - `"(Let "` → parse name + value-type from line; extend env with `name:value-type`; return value-type.
  - `"(Ret "` → parse inner type from line; return it.
  - `"(If "` → check cond is `"Bool"`, then/else types compatible (equal or either `"Unknown"`); return then-type.
  - `"(Arithmetic "` → both operands must be `"Int"` or both `"Str"` (for `+`); return operand type.
  - `"(FnCall "` → return `"Unknown"` (arity checking deferred — no fn type env yet).
  - `"(FnDef "` → parse params, extend env, check body; return declared return type or body type.
  - default → `"Unknown"`.
- **Env-lookup helper:** scan `\n`-joined env string for line starting with `name:`, extract type suffix.
- **Types compatible:** `"Int"` ≡ `"Int"`, `"Unknown"` ≡ anything (inference gap accepted).
- **Error on mismatch:** `(+ "ERROR: type mismatch — expected " (+ expected-type (+ " got " actual-type)))`.

**Note on AST nesting.** S2.2 parser emits flat single-line AST nodes (e.g. `(Let x (Int 42))`). The checker parses these inline: extract sub-expressions by scanning for balanced parens within the line. Add helpers: `extract-inner :: line:Str start:Int -> Str` (returns content between first `(` and matching `)`) and `split-fields :: s:Str -> (Str, Str)` (splits on first space).

**Tests to add** (hand-traced, source-level):
- `ok-let.ast` + `ok-let.verdict`: `(Let x (Int 42))` → `PASS`.
- `ok-arith.ast` + `ok-arith.verdict`: `(Arithmetic Add (Int 1) (Int 2))` → `PASS`.
- `ok-if.ast` + `ok-if.verdict`: `(If (Bool true) (Int 1) (Int 0))` → `PASS`.
- `err-arith.ast` + `err-arith.verdict`: `(Arithmetic Add (Int 1) (Str "x"))` → `ERROR: type mismatch`.
- `err-if-cond.ast` + `err-if-cond.verdict`: `(If (Int 1) (Int 2) (Int 3))` → `ERROR: if condition must be Bool`.
- `ok-var.ast` + `ok-var.verdict`: `(Let x (Int 10))\n(Var x)` → `PASS`.

**Definition of done.**
- `check.aven` parses under seed interpreter (source-level verified, no `|>`, binary `+` only).
- `check-node` dispatches on: `Int`, `Float`, `Str`, `Bool`, `Var`, `Let`, `Ret`, `If`, `Arithmetic`, `FnCall`, `FnDef`.
- `env-lookup` and `env-extend` correctly thread environment across `Let` bindings.
- All 6 fixtures exist with hand-traced verdicts.
- Type-mismatch errors reported for: non-Bool `@if` condition; mixed-type arithmetic.

**Out of scope.** Effect arrows, `@uncertain` tracking, `@match`, `@mod`/`@use`, full bidirectional inference, inline `FnDef` body type unification (body-vs-declared return type check), generics, runtime parity (QUEUED).

---

### S2.4 — Module resolver in AVEN — **Done**

**Outcome.** `aven-core/module.aven` implements capability-based module resolution (10 functions). Registry threaded as `\n`-joined `modname:caps\n` string; `registry-extend` prepends for shadow semantics. `caps-contain` correctly matches exact comma-delimited tokens (no prefix false-positives). `resolve` returns `PASS` or `ERROR:module-not-found:X` / `ERROR:cap-not-exported:X`. Opus approved Round 1 (clean first pass). 5 golden fixtures. Runtime parity QUEUED.

## Active Stage spec (archived): S2.4 — Module resolver in AVEN

**Goal.** Implement capability-based module resolution in `aven-core/module.aven`. A module registry maps module names to their exported capabilities. The resolver checks that `@use [cap1, cap2] from modname` requests only a subset of what `modname` exports. No DAG or topological sort (deferred — requires list structures); this stage establishes the module identity + capability verification layer.

**Files touched.**
- `aven-core/module.aven` — resolver implementation (replaces stub).
- `aven-core/tests/module-fixtures/*.query` — 5 test query files (new).
- `aven-core/tests/module-fixtures/*.verdict` — 5 verdict files (new).

**Registry representation.** A `\n`-joined string of `modname:cap1,cap2,cap3\n` entries. Example: `"fs:read,write,list\nhttp:get,post\n"`. A module with no capabilities: `"math:none\n"`. The registry is threaded as a parameter; no global state.

**Capability string representation.** Capabilities within a module entry are `,`-joined. Example `"read,write,list"`. A query `"read,write"` is a subset of `"read,write,list"`.

**Specific changes — `module.aven`:**
- `@fn resolve :: registry:Str modname:Str requested:Str -> Str` — entry point; looks up `modname` in registry, checks every cap in `requested` is present in the module's cap list; returns `"PASS"` or `"ERROR:cap-not-exported:<cap>"`.
- `@fn registry-lookup :: registry:Str modname:Str -> Str` — scans `\n`-joined registry for `modname:...`, returns caps string or `"NOT_FOUND"`.
- `@fn registry-extend :: registry:Str modname:Str caps:Str -> Str` — prepends `modname:caps\n` to registry (same shadow-first pattern as S2.3 env-extend).
- `@fn caps-contain :: caps:Str needle:Str -> @Bool` — checks if `needle` appears as a `,`-delimited entry in `caps`.
- `@fn check-caps :: caps:Str requested:Str -> Str` — iterates over `,`-split `requested`, calls `caps-contain` for each; returns `"PASS"` or `"ERROR:cap-not-exported:<first-missing-cap>"`.
- Iteration helpers (no list type in seed-AVEN; use position-based recursion):
  - `@fn next-cap :: s:Str pos:Int -> Str` — returns next `,`-delimited token starting at `pos`.
  - `@fn find-comma :: s:Str pos:Int len:Int -> Int` — returns position of next `,` or `len` if none.
  - `@fn skip-cap :: s:Str pos:Int -> Int` — returns position after current cap (past `,` or at `len`).

**Token format for query files.** Each `.query` file contains:
```
registry: fs:read,write,list\nhttp:get\n
module: fs
caps: read,write
```
Three `\n`-separated lines: registry string, modname, requested caps. The resolver reads these three fields.

**Tests to add** (hand-traced):
- `ok-subset.query` + `ok-subset.verdict`: `fs` exports `read,write,list`; request `read,write` → `PASS`.
- `ok-single.query` + `ok-single.verdict`: `http` exports `get,post`; request `get` → `PASS`.
- `err-missing.query` + `err-missing.verdict`: `fs` exports `read`; request `read,write` → `ERROR:cap-not-exported:write`.
- `err-unknown-mod.query` + `err-unknown-mod.verdict`: module `db` not in registry → `ERROR:module-not-found:db`.
- `ok-exact.query` + `ok-exact.verdict`: exact cap match (all caps requested, all present) → `PASS`.

**Definition of done.**
- `module.aven` implements all 7 functions; no `|>`, all `+` binary only.
- `resolve` returns `PASS` for valid capability subsets and `ERROR:...` for violations.
- `registry-lookup` returns caps string or `NOT_FOUND`.
- `caps-contain` correctly identifies whether a cap is in a `,`-joined list.
- All 5 fixtures present with hand-traced verdicts.

**Out of scope.** DAG cycle detection, topological sort, `@pub` export enforcement, `@ctx` threading, multi-file module loading (all deferred to S2.7 driver).

---

### S2.5 — `@diff` engine in AVEN — **Done**

**Outcome.** `aven-core/diff.aven` implements selector-based AST-line replacement (11 functions). Selectors `"fn name"` match `(FnDef name ...)` and `"let name"` match `(Let name ...)`. `scan-and-replace` threads `found:@Bool` to replace only the FIRST match — Haiku reviewer caught that subsequent matching lines were also replaced (fixed: guard with `bool_and(bool_not found, matches-selector)`). `if-acc-empty` handles leading-newline correctly. Opus approved Round 1. 4 golden fixtures. Runtime parity QUEUED.

---

## Active Stage: S2.6 — Evaluator in AVEN

**Goal.** Implement a tree-walking evaluator in `aven-core/eval.aven` that consumes a canonical AST string (S2.2 parser output, `\n`-joined S-expressions) and produces a result value string. Scope: evaluate literals, `Var` lookup, `Let` binding, arithmetic (`+ - * /`), `If`, `Ret`, and `FnCall` (user-defined functions in env). The evaluator threads an environment string (same `name:value\n` pattern from S2.3). No closures in AVEN output; all iteration via recursive `@fn`.

**Files touched.**
- `aven-core/eval.aven` — evaluator implementation (replaces stub).
- `aven-core/tests/eval-fixtures/*.ast` — 5 AST input files (new).
- `aven-core/tests/eval-fixtures/*.value` — 5 expected value files (new).

**Value representation.** Results are strings. Primitives: `"42"` (Int), `"3.14"` (Float), `"hello"` (Str — raw, no quotes), `"true"`/`"false"` (Bool). No compound value types for S2.6 scope.

**Environment representation.** Same as S2.3: `\n`-joined `name:value\n` entries, prepended on extend (newest first). Values stored as their string form.

**Specific changes — `eval.aven`:**
- `@fn eval :: ast:Str -> Str` — entry: calls `eval-block ast ""`, returns last computed value.
- `@fn eval-block :: ast:Str env:Str -> Str` — processes lines sequentially; for `Let` bindings, threads new env; returns final value.
- `@fn eval-node :: line:Str env:Str -> (Str, Str)` — returns `(value, updated-env)` for one AST line. Dispatch on node kind:
  - `(Int N)` → `"N"` (the integer string).
  - `(Float F)` → `"F"`.
  - `(Str "s")` → `"s"` (strip outer quotes).
  - `(Bool true)` / `(Bool false)` → `"true"` / `"false"`.
  - `(Var name)` → `env-lookup env name` (from S2.3 pattern, re-implemented here).
  - `(Let name expr)` → eval expr, extend env with `name:value`, return value.
  - `(Ret expr)` → eval inner expr, return value.
  - `(If cond then else)` → eval cond; if `"true"` eval then, else eval else.
  - `(Arithmetic op left right)` → eval both, apply op (int arithmetic for Int values).
  - default → `"unknown"`.
- **Arithmetic**: parse both operand values as integers via `str-to-int` helper (digit-by-digit), apply op, format back to string with `int-to-str`. Support `Add`/`Sub`/`Mul`/`Div`.
- **Sub-expression evaluation**: `(Let x (Int 42))` — `node-content` gives `"x (Int 42)"`. Split on first space to get name; the rest is the sub-expression line. Call `eval-node` recursively on the sub-expression line.

**Helper functions:**
- `str-to-int :: s:Str -> Int` — convert digit string to Int (recursive, position-based).
- `int-to-str :: n:Int -> Str` — convert Int to digit string (recursive, divide-by-10).
- Reuse `peek`/`advance`/`find-newline`/`starts-with`/`node-kind`/`node-content`/`find-space` (copy from prior stages).
- `env-lookup`/`env-extend` (same pattern as S2.3, re-implemented).

**Tests to add** (hand-traced):
- `lit-int.ast` + `lit-int.value`: `(Int 42)` → `"42"`.
- `arith.ast` + `arith.value`: `(Arithmetic Add (Int 3) (Int 4))` → `"7"`.
- `let-var.ast` + `let-var.value`: `(Let x (Int 10))\n(Var x)` → `"10"`.
- `if-true.ast` + `if-true.value`: `(If (Bool true) (Int 1) (Int 0))` → `"1"`.
- `nested.ast` + `nested.value`: `(Arithmetic Mul (Arithmetic Add (Int 2) (Int 3)) (Int 4))` → `"20"`.

**Definition of done.**
- `eval.aven` complete; no `|>`, all `+` binary only.
- `eval-node` dispatches on: Int, Float, Str, Bool, Var, Let, Ret, If, Arithmetic.
- `str-to-int` and `int-to-str` work for non-negative integers (negative TBD).
- All 5 fixtures present with hand-traced values.
- `eval-block` correctly threads env across `Let` bindings so `Var` lookup works.

**Out of scope.** FnDef/FnCall evaluation (no closure support needed for S2.6 scope), `@match`, stdlib bridging, negative integer parsing, floating-point arithmetic.

---

### S2.6 — Evaluator in AVEN — **Done**

**Outcome.** `aven-core/eval.aven` implements a tree-walking evaluator (31 functions) consuming canonical AST strings. Threads `name:value\n` environment via `env-extend`/`env-lookup`. Evaluates: Int/Float/Str/Bool literals, Var lookup, Let binding, Ret, If, Arithmetic (+/-/*//). Integer ↔ string conversion via `str-to-int`/`int-to-str` (digit-by-digit loops). `scan-to-close` correctly handles nested parentheses for nested arithmetic evaluation. Opus approved Round 1 (clean pass). 5 golden fixtures. Runtime parity QUEUED.
