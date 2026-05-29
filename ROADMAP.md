# AVEN Roadmap

This is the near-term sequencing of work. `plan.md` is the long-form architecture and milestone reference; this file is the actionable sprint plan derived from it.

The principle: AVEN's headline features — `@diff` patches, `@uncertain`/`@intent`/`@ctx` AST nodes, capability-gated imports, effect-typed arrows — are the **point** of the language, not polish at the end. The roadmap surfaces them at their actual milestone home rather than burying them in a "future work" list.

---

## Where we are

**M1–M7 complete.** The seed crate at `seed/` lexes, parses, evaluates, and type-checks the full AVEN v0.1 subset including modules, `@diff` engine, `@match`, `@err`, `@uncertain` enforcement, `@intent` indexing, `@ctx` API, and effect-typed arrows. All items in the development queue are done.

Key accomplishments (cumulative):
- M1: Lex/parse/eval for core expressions, source spans, `NodeId`
- M2: Bidirectional type checker, orthogonal `EffectSet`, structural types, capability markers
- M3: `@match` with pattern binding, `@err` values, typed `@uncertain` enforcement, `@intent` indexing
- M4: Module system (`@mod`/`@use`/`@pub`), dotted module identity, capability gating, DAG cycle detection, topo-ordered type checking, real `@ctx` API
- M5: `@diff` engine — selector path parser, structured `DiffOp` nodes, all 5 operations (`@replace`/`@insert`/`@delete`/`@move`/`@copy`), entity-name selectors, `@diffs` atomic batch with rollback, post-patch type validation, `@meta` blocks, `.avenpatch` serialization
- M6: All 8 stdlib modules done — `aven/std/io`, `aven/std/fs`, `aven/std/http`, `aven/std/str`, `aven/std/json`, `aven/std/time`, `aven/std/math`, `aven/std/collections`
- M7: All 7 items done — span-aware errors, `aven fmt`, spec coverage audit, non-trivial AVEN program (JSON parser), `@uncertain` deploy blocker, `aven repl`, `@intent` index dump

### Stable build (for reproducibility)

```text
$ rustc --version
rustc 1.95.0 (59807616e 2026-04-14)
$ cargo --version
cargo 1.95.0 (f2d3ce0bd 2026-03-21)
```

Last verified: 436 tests passing (`cargo build && cargo test`, 2026-05-25). Zero warnings.

---

## Milestone ladder

### M1 — Core lex/parse/eval

Status: **Complete.** All items done. Build verified at 70 tests (re-verify after NodeId/Type stages pending).

| Done | Item |
|---|---|---|
| ✅ | Lexer: sigil tokens, keywords, ints (incl. `1_000`), strings (with escapes), symbols, `@true`/`@false` |
| ✅ | Parser: `@fn`, `@let`, `@ret`, `@call`, `@if`/`@then`/`@else`, parenthesized binary ops (Lisp-prefix), `@io.write` |
| ✅ | Eval: tree-walking, `Env` with parent scope, closure capture |
| ✅ | Annotation nodes: `@intent`, `@uncertain`, `@ctx` (parse-only, no-op at runtime) |
| ✅ | `@diff` keyword stub: `@diff`/`@diffs`/`@replace`/`@insert`/`@delete`/`@move`/`@copy`/`@meta` reserved at the lexer; `Expr::Diff` appears in the AST; parser swallows the body; eval returns `Nil`. Full sub-grammar is M5. |
| ✅ | `parse_fn_def` body uses `parse_statement` so `@ret` is legal at body head |
| ✅ | **Verify**: `cargo build` and `cargo test` pass on host with Rust 1.95.0 (59 tests, 0 failures) |
| ✅ | Variable lookup in arithmetic operands — `@let x :: 10 (+ x 5)` returns `15`; tested in `test_var_in_arithmetic` and `test_var_in_nested_arithmetic` |
| ✅ | Source spans on tokens and AST nodes — `SourceSpan` on every `Expr` variant except `Nil`; lexer exposes `tokenize_spanned`; parser attaches byte-range spans |

### M2 — Types, effects, capabilities

The point of AVEN's type system: effects in the arrow, capabilities at the import boundary.

| Done | Item | Notes |
|---|---|---|
| ✅ | Effect arrow tokens (orthogonal) | `EffectArrow(EffectSet)` in lexer; `EffectSet` has `err`/`io`/`async_` flags; 8 arrows: `->`, `-?>`, `-!>`, `-~>`, `-?!>`, `-?~>`, `-!~>`, `-?!~>` |
| ✅ | Type representation in AST | `Type` enum, `PrimitiveType`, `EffectSet`, `UnionVariant`, `CapabilityMarker` in `ast.rs` |
| ✅ | Type annotations in `Expr::FnDef` | `params: Vec<(String, Option<Type>)>`; `return_type: Option<Type>`; `cap: Vec<CapabilityMarker>`; `effect_level: EffectSet` |
| ✅ | Bidirectional type checker | `typechecker.rs`; `typecheck(expr, env) -> Result<Type, TypeError>`; covers M1 forms + effect subset enforcement at call sites |
| ✅ | Record + union + option + list types | `types_compatible()` helper in `typechecker.rs`; `FnDef` handler accepts compound annotations; 13 unit tests + 5 integration tests |
| ✅ | Effect enforcement at call sites | `EffectSet::is_subset_of()` — callee effects must be a subset of caller's; Pure cannot call IO/Async |
| ✅ | `@cap` markers | `@cap [read write]` on `@fn` definitions; `cap: Vec<CapabilityMarker>` in `Expr::FnDef` |
| ✅ | Capability gating at `@use` (Phase 1 — parse stub) | `@use [caps] @from module` parses into `Expr::Use`; eval returns Nil; M4 enforcement deferred |

### M3 — Control flow + match + err + annotation enforcement

| Done | Item | Notes |
|---|---|---|
| ✅ | `@match` | Symbol-tag match, value destructure, wildcard `_` |
| ✅ | `@err` | Returns an error *value*, not a thrown exception; types as `#ok T \| #err E` |
| ✅ | Typed `@uncertain` | The checker requires an explicit acknowledgement when an `@uncertain` value crosses a typed module boundary |
| ✅ | `@intent` indexing | Build a side table from `@intent` nodes for tooling lookup by selector path |

### M4 — Module system

This is where `@ctx` becomes a real feature, not a placeholder.

| Done | Item | Notes |
|---|---|---|
| ✅ | `@mod`, `@use`, `@pub` | Multi-file compilation |
| ✅ | Dotted module identity | `aven/std/io`, `app/services/auth` |
| ✅ | Capability verification | At import resolution; reject `@use [write]` if target is `@pub [read]` only |
| ✅ | DAG check | Reject circular module graphs |
| ✅ | Topo-ordered type checking | Dependencies first |
| ✅ | Real `@ctx` API | `@ctx.get ctx "db"` returns `?T`; threaded through function args, never global |

### M5 — `@diff` engine

The AI-edit-loop primitive. The single most leveraged feature for agent workflows.

**Already in place from M1:** the keyword tokens (`@diff`, `@diffs`, `@replace`, `@insert`, `@delete`, `@move`, `@copy`, `@meta`) are reserved at the lexer and `Expr::Diff` appears in the AST. The M1 parser swallows the body; M5 replaces that stub with real grammar.

| Done | Item | Notes |
|---|---|---|---|
| ✅ | `NodeId` on every AST node | `pub type NodeId = u64`; every `Expr` variant except `Nil` carries `node_id`; `Parser` has a per-parse counter |
| ✅ | Selector path parser | `/fn greet/body/ret`, positional disambiguation `/let[0]` |
| ✅ | Structured `DiffOp` nodes | `Diff { ops: Vec<DiffOp> }`; `DiffOp { kind, selector, body }` |
| ✅ | Operations | `@replace`, `@insert` (with `first` / `last` / `before:` / `after:`), `@delete`, `@move`, `@copy` |
| ✅ | Entity-name selectors + spec §2.7 compliance | `/fn greet/body/ret` entity resolution, proper Move/Copy semantics |
| ✅ | Post-patch validation | Re-run type checker; if it fails, roll back the whole batch |
| ✅ | Rollback + atomic `@diffs` batch | All-or-nothing; clone-and-swap on first failure |
| ✅ | `@meta` blocks | Parsed but stored as metadata (description, author, timestamp) — does not affect application |
| ✅ | `.avenpatch` files | Serialized patch sets for off-line composition |

This milestone is where AVEN starts paying for itself: agents send `@replace /fn greet/body` + 3 lines instead of 200-line file rewrites.

### M6 — Standard library intrinsics

`NativeFn` values registered in the initial environment. No FFI — stdlib functions are just Rust closures.

| Done | Module | Capabilities |
|---|---|---|
| ✅ | `aven/std/io` | `[print read write]` |
| ✅ | `aven/std/fs` | `[read write list]` |
| ✅ | `aven/std/http` | `[get post put delete]` |
| ✅ | `aven/std/str` | `[len get sub find rest trim to_int from_int eq]` |
| ✅ | `aven/std/json` | `[parse serialize]` |
| ✅ | `aven/std/time` | `[now sleep format]` |
| ✅ | `aven/std/math` | `[calc]` |
| ✅ | `aven/std/collections` | `[list map set]` |

### M7 — Polish + self-hosting prep

| Done | Item | Notes |
|---|---|---|
| ✅ | Span-aware error messages | Show source location, caret, hint |
| ✅ | `aven fmt` | Canonical re-printer; output is identical to the parsed AST |
| ✅ | `aven repl` | Multi-line input, history, prompt continuation |
| ✅ | Full spec coverage audit | Every section of `AVEN_SPEC.md` exercised by at least one test |
| ✅ | Non-trivial AVEN program | A JSON parser written in AVEN, compiled and run by the seed |
| ✅ | `@uncertain` deploy blocker | CI linter rule rejecting modules with `@uncertain` |
| ✅ | `@intent` index dump | `aven intent <module>` prints the annotations |

After M7 the Rust seed is stable enough to start writing the AVEN compiler in AVEN (Stage 2).

---

## Stage-level horizon

| Stage | What | Lives in |
|---|---|---|
| **Stage 1** | Rust seed interpreter, M1–M7 | `seed/` (this repo) |
| **Stage 2** | Self-hosted AVEN compiler, written in AVEN, compiled by the seed | `aven-core/` (future, may be a separate repo per project instructions) |
| **Stage 3a** | `aven-native` — AVEN → LLVM, production runtime | Stage 2+ |
| **Stage 3b** | `aven-llm` — agent-driven dry-run interpreter, structured execution traces, no determinism guarantee | Stage 2+ |

The repo split into `aven-spec` / `aven-seed` / `aven-core` happens when the seed can compile real programs (end of M7), per the project instructions. Until then this stays a monorepo.

---

---

## Immediate next actions (in order)

### POC Phase — aven-guard (AI-Brake proof-of-concept)

1. ~~**Re-verify seed build** — Run `cargo build && cargo test` on `seed/` to confirm 479 tests pass with zero warnings. Blocks all downstream POC work.~~ **Done.** 436 tests passing, zero warnings (2026-05-25).

2. ~~**Add `aven verify` subcommand** to `seed/src/main.rs` — the `--verify` flag from the POC doc. Reads an `.aven` file, runs parse + typecheck + check_uncertainty + capability gating in one pass. Exits 0 on pass, non-zero with structured diagnostics on fail. Produces human-readable and JSON output.~~ **Done.** `run_verify()` in main.rs. 10 tests (5 library + 5 binary spawn). 445 tests passing. Opus approved Round 3.

3. ~~**Build `aven_bridge.py`**~~ **Done.** `AVENExecutionEngine` with `verify_source`/`verify_file`/`_run_verify`. 20 tests passing. Opus approved after 3 rounds (type validation, file field normalization, JSON contract robustness). 2026-05-25.

4. ~~**Create demo fixtures**~~ **Done.** `demo/` with `clean.py`, `uncertain.py`, `capability_violation.py`, `run_poc.sh`. `bash demo/run_poc.sh` exits 0. Opus approved R3. 2026-05-26.

5. ~~**Package as `pip install aven-guard`**~~ **Done.** `pyproject.toml` + `aven_guard/` package with `_engine.py`, `_binary.py`, `cli.py`, `__main__.py`. `aven-guard check --source "(+ 1 2)"` → PASS. Opus approved R3. 2026-05-26.

**After POC validation → proceed to production build** (4-stage plan in `aven-guard-plan.md`).

### Production Phase — aven-guard (from aven-guard-plan.md)

6. **Stage 1** — Python→AVEN bridge with stdlib AST parsing, uncertainty/effect detection, JSON output (Python-only, no tree-sitter).

7. **Stage 2** — TypeScript→AVEN bridge (tree-sitter required; or skip to Stage 3 if descoped).

8. **Stage 3** — Capability gating for Python (import→@use mapping + M7 verification).

9. **Stage 4** — VS Code extension: onAccept heuristic, sidecar spawning, Problems panel output.

---

## Active Stage: Production Stage 1 — Python→AVEN Bridge (AST-based)

**Goal.** Bridge Python source code to AVEN verification engine using only Python stdlib `ast` module (no tree-sitter dependency). Detect `eval()` / `exec()` / `subprocess` calls and map them to AVEN `@uncertain` / effect annotations. Generate minimal AVEN source string capturing these semantics and run `AVENExecutionEngine.verify_source()` to collect violations. Return structured JSON for integration into tooling.

**Scope decision.** Pragmatic: extend the existing Python package (`aven_guard/`) rather than create a new Rust crate. The seed binary (`aven verify`) already handles all AVEN verification; the production bridge is a translator + orchestrator, not a compiler. Deferring the Rust crate to Stage 2 when complexity warrants it (e.g., multi-language dispatch, caching, performance). This keeps Stage 1 to ~200 lines Python.

**Toolchain.** `tree-sitter` Python library is **not installed**. Decision: **do not use tree-sitter in Stage 1.** Instead, use stdlib `ast` module (Python 3.8+, zero dependency) to walk the Python AST and detect unsafe patterns. This is sufficient for MVP: `eval()`, `exec()`, `subprocess.*`, and `__import__()` are AST-level function calls, not requiring syntax-level information. Stage 2 can add `tree-sitter` if fine-grained position tracking or TypeScript support is required.

### Files to create/modify

1. **`aven_guard/_python_bridge.py`** (new) — core translator
   - `PythonToAVENBridge` class with `source_to_aven_string(python_code: str) -> str` method
   - Walk Python AST via `ast.parse()`, detect patterns, emit AVEN source
   - Pattern detection:
     - `Call` nodes with `func.id` in `['eval', 'exec', '__import__', 'compile', 'getattr']` → wrap payload in `@uncertain`
     - `Call` nodes with `func.attr == 'call'` and `func.value.id == 'subprocess'` → emit effect marker + `@uncertain`
     - `Raise` statements → emit effect marker (error)
     - `Import` / `ImportFrom` statements → emit `@use` declarations with capability mapping
   - Map Python import names to `python/<module>` paths (e.g., `import os` → `@use [read write list] @from python/os`)
   - Emit minimal valid AVEN: wrap detected violations in a `@fn main` body, escape all string payloads
   - Return AVEN source string; on parse error return fallback `"@uncertain 42"` (triggers check failure for diagnostics)

2. **`aven_guard/_capability_map.py`** (new) — import-to-capability lookup
   - `PYTHON_IMPORT_CAPS` dict mapping module name → capability list
   - Examples: `"os"` → `["read", "write", "list"]`, `"subprocess"` → `["exec"]`, `"json"` → `["read"]`
   - Covers 15 common stdlib modules + fallback `["read"]` for unknown stdlib

3. **`aven_guard/_check.py`** (modify existing or new) — orchestrator
   - `check_python_source(source: str) -> dict` function
   - Call `PythonToAVENBridge.source_to_aven_string(source)`
   - Pass result to `AVENExecutionEngine.verify_source(aven_src)`
   - Extract violations from `{"pass": bool, "errors": [...]}`
   - Map each error back to original Python source line/col (use `ast.parse(source)` node positions)
   - Enrich each violation with `check_type` ("uncertainty" | "capability" | "effect")
   - Return JSON: `{"file": "<source>", "violations": [{"line": N, "col": C, "check": T, "message": M}, ...]}`

4. **`aven_guard/cli.py`** (modify) — add Python check command
   - Extend `argparse` to add `--lang python` option (or default to `python` if file ends in `.py`)
   - Route `aven-guard check <file> --lang python` to `check_python_source()`
   - Output JSON (already implemented for `--json` flag)

5. **`tests/test_python_bridge.py`** (new) — integration tests
   - `test_eval_detected_as_uncertain` — Python source with `eval()` → AVEN source includes `@uncertain`
   - `test_subprocess_call_flagged` — `subprocess.run()` → error in violations
   - `test_import_os_mapped_to_caps` — `import os` → `@use [read write list] @from python/os`
   - `test_mixed_violations` — source with both `eval()` and `import subprocess` → two violations
   - `test_clean_source_passes` — safe Python code (no eval, no subprocess) → `pass: true`
   - `test_syntax_error_in_python_source` — invalid Python → graceful error in violations (not crash)

### Acceptance Criteria

- ✅ `PythonToAVENBridge.source_to_aven_string(source)` generates valid AVEN source for any Python input
- ✅ Pattern detection covers: `eval()`, `exec()`, `__import__()`, `compile()`, `getattr()`, `subprocess.*`, `import` statements
- ✅ Capability map includes ≥12 stdlib modules with correct mappings
- ✅ `check_python_source(source)` returns JSON with `violations` array, each entry has `line`, `col`, `check`, `message`
- ✅ All 6 integration tests pass
- ✅ No new dependencies (stdlib `ast` only; `aven_guard` still zero-dep except the seed binary)
- ✅ `aven-guard check myfile.py` works end-to-end (reads file, emits JSON Lines to stdout)
- ✅ `cargo build` (seed) + `python -m aven_guard check --help` both work (no regressions)

### Definition of Done

- 200–250 lines of Python code across 3 new files
- 6 integration tests, all passing
- `aven-guard` CLI accepts `--lang python` (or auto-detects `.py` extension)
- Violations JSON includes file/line/col/check/message
- Opus approval after ≤2 rounds (quality bar: correctness, no edge-case crashes, clear bridge semantics)

### Out of Scope

- Tree-sitter integration (deferred to Stage 2)
- Rust crate (deferred to Stage 2 when complexity warrants)
- TypeScript support (Stage 2)
- Capability enforcement via `@pub` / `@cap` markers (Stage 3)
- VS Code extension (Stage 4)
- Fine-grained type mapping (Python type hints → AVEN Type structs — use `Type::TypeParam("Any")` for now)

---

## Completed Stages

### POC Item 5 — pip-installable aven-guard package — **Done**

**Outcome.** Created minimal zero-dependency Python package with pyproject.toml, aven_guard/ directory containing __init__.py (exports AVENExecutionEngine), _engine.py (copy of AVENExecutionEngine from aven_bridge.py using locate_binary() fallback), _binary.py (locate_binary checks AVEN_BINARY env var then shutil.which), cli.py (argparse-based main with `check` subcommand supporting --source, file arg, --json flag), and __main__.py (enables python -m invocation). Tests: `python -m aven_guard check --source "(+ 1 2)"` passes (exits 0, prints PASS), `python -m aven_guard check --source "@uncertain 42"` fails (exits 1, prints FAIL with stage/message), `--json` flag outputs raw dict. `from aven_guard import AVENExecutionEngine` works. 2026-05-26.

### POC Item 4 — Demo fixtures — **Done**

**Outcome.** `demo/` directory with `clean.py` (safe IO fixture with `AVEN_CAPS`), `uncertain.py` (eval() fixture), `capability_violation.py` (subprocess fixture), and `run_poc.sh` (bash runner). Runner passes AVEN source via `AVEN_SOURCE` env var to avoid heredoc injection, captures stderr to temp file with explicit cleanup, TTY-guarded ANSI colors, failure diagnostics to stderr, `.get()` on error dict keys. `bash demo/run_poc.sh` exits 0. 3 Opus rounds: R1 found 8 issues (label honesty, masked errors, injection, dict repr, stderr routing, TTY, temp leak), R2 found 3 more (hard key indexing, non-local vars, dangling trap), R3 approved. 2026-05-26.

### POC Item 3 — aven_bridge.py — **Done**

**Outcome.** `AVENExecutionEngine` Python class in `aven_bridge.py`: `verify_source(source_str)` writes temp `.aven` file, shells to `aven verify`, returns `{"file": "<source>", "pass": bool, "errors": [...]}`. `verify_file(path)` verifies existing file, normalizes `file` field to canonical path. `_run_verify(path)` handles JSON parsing with full contract validation: required-key presence, `pass` must be `bool`, `errors` must be `list`, `returncode`/`pass` mismatch detection, exit-0-no-JSON contract violation, `TimeoutExpired`/`OSError` handling with stage-tagged errors. Binary lookup: `AVEN_BINARY` env var → `binary_path` arg → `shutil.which("aven")`; raises `RuntimeError` if not found or path doesn't exist. 20 tests in `test_aven_bridge.py`. Opus approved after 3 rounds. 2026-05-25.

### M5.7 — Post-patch type checking — **Done**

**Outcome.** `EvalError::TypecheckFailed(String)` variant added to `eval.rs` with Display impl. `diffs_apply` extended: after `apply_diff` succeeds on the cloned AST, calls `crate::typechecker::typecheck(&clone, &TypeEnv::new())`; on `Err(type_err)` returns `EvalError::TypecheckFailed(type_err.message)` and discards the clone (implicit rollback); on `Ok(_)` commits the clone to root as before. This enforces spec §2.7 "the result must be a valid AVEN module (type-checked)" — patches that break type correctness are rejected atomically. 3 new unit tests. 242 tests passing (133 unit + 109 integration), zero warnings. 2 Opus rounds: Round 1 found that both "typecheck fails" tests were actually testing the `apply_diff` failure path (invalid selector), never reaching the `TypecheckFailed` branch; Round 2 approved after tests were rewritten to use `Expr::Bool` replacing an `Arithmetic(Int,Int)` operand (apply_diff succeeds, typecheck correctly rejects the resulting type-invalid AST).

### M5.6 — Entity-name selectors and spec §2.7 Move/Copy compliance — **Done**

**Outcome.** `find_node_by_selector` extended with entity-name matching: when `PathSegment::Named` doesn't match a structural field and the current node is a `Block`, the function now scans Block children for a `FnDef`/`Let`/`Mod` whose `.name` matches the segment. This makes `/fn greet`, `/let x`, and nested paths like `/fn f1/body/let y` resolve correctly. Move and Copy handlers refactored to treat `InsertMode::Before(path)`/`After(path)` as anchor-sibling paths (per spec §2.5) that must exist; the clone is inserted at `anchor_index` (Before) or `anchor_index+1` (After) inside the anchor's parent Block. The old "destination must not exist" check was replaced with "anchor must exist" validation. `test_move_destination_exists_rejects` was retained and documents that a pre-existing node at the target index triggers an error. 9 new unit tests. 239 tests passing (130 unit + 109 integration), zero warnings. 3 Opus rounds: Round 1 found After/Before indistinguishable and missing source-not-found test; Round 2 found After branch was dead code (destination-must-not-exist + After+1 made valid input impossible); Round 3 approved after anchor-semantics fix and new `test_copy_insert_after_offsets_by_one`.

**Workflow notes.** Opus quality gate required 3 rounds (the maximum). Round 1 filtered 8 of 12 concerns as pre-accepted design decisions or out of scope; passed #2 (After no +1), #9 (source-not-found test), #10 (After untested). Round 2 caught that the Round 1 fix left the After branch dead (logical impossibility: anchor-must-not-exist combined with After+1 always OOB). Round 3 approved after semantic flip to anchor-must-exist. The M5.5 pre-accepted simplifications (source→Nil, First/Last not supported for Move/Copy) remain in place and are documented.

### M5.5 — @insert, @move, @copy operations + @diffs atomic batch — **Done**

**Outcome.** `apply_diff()` extended to handle all five `DiffKind` variants. `Insert`: validates target is `Block`, splices payload using `InsertMode::First`/`Last` (prepend/append) or `Before(idx)`/`After(idx)` (numeric-index positional insertion). `Move`: extracts source via selector, clones it, sets source to `Expr::Nil`, parses destination path from `insert_mode` (`Before`/`After` string), overwrites destination with source clone. `Copy`: same as Move but does not set source to Nil. `diffs_apply(root, ops)` added: clones root, calls `apply_diff` on clone, commits clone to root on success, leaves root unchanged on any failure (rollback). 9 unit tests (8 planned + 1 from Opus Round 2 atomicity gap). 230 tests passing (121 unit + 109 integration), zero warnings. 2 Opus rounds: Round 1 found Move/Copy requiring dummy payload (confusing error) and missing atomicity test for "first-op-succeeds-then-rollback"; Round 2 approved after fixes.

**Workflow notes.** Opus Round 1 filtered: Move/Copy "replace dest with clone" semantics vs spec §2.7 "dest must not exist" — accepted as M5.5 simplification per plan; spec violation noted for M5.6. Before/After numeric-index encoding (vs selector path per spec §2.5) — accepted as pre-approved deviation. 11 of 13 Opus concerns filtered. 2 passed: payload gate (#4) and atomicity test gap (#8). Follow-up queued: M5.6 — entity-name selectors, proper Move/Copy semantics, spec §2.7 full compliance.

### M5.4 — Diff evaluation engine — **Done**

**Outcome.** `find_node_by_selector<'a>(expr: &'a mut Expr, selector: &SelectorPath) -> Option<&'a mut Expr>` added to `eval.rs` — recursive descent matching `PathSegment::Named` by structural field name (left/right/value/body/cond/then_branch/else_branch/ret/write/uncertain/key/scrutinee/payload) and `PathSegment::Index(n)` into `Block` vectors. `apply_diff(expr: &mut Expr, ops: &[DiffOp]) -> Result<(), EvalError>` added — iterates ops, calls selector walker, applies Replace (swap with payload clone) or Delete (set to Expr::Nil); returns `InvalidOperation` if selector not found or payload missing. `Expr::Diff` eval arm updated to return `Ok(Value::Nil)` directly (apply_diff is the internal API, not called from eval — design note: standalone `@diff` has no root target to apply to; mutations are observable only via direct `apply_diff` calls). 6 unit tests + 5 integration tests. 221 tests passing (112 unit + 109 integration), zero warnings. 2 Opus rounds: round 1 found that eval arm was calling apply_diff on a clone of itself (circular) and discarding result; round 2 approved after fix + Index unit tests added.

**Workflow notes.** Opus quality gate triggered twice. Round 1 found eval arm calling apply_diff on the Diff node itself (nonsensical — Diff node has no Arithmetic children, selectors always failed, clone discarded). Orchestrator filtered concerns #3, #4 (spec entity-name selectors — deferred), #5 (Delete-as-Nil — plan-accepted), #7 (rollback — deferred to M5.5), #8 (style), #10 (FnDef param selection — out of scope). Passed #1+#6+#2+#9. Round 2: APPROVED.

**Follow-up queued.** M5.5 — `@insert`, `@move`, `@copy` diff operations; `@diffs` atomic batch with rollback.

### M5.1 — Selector path parser — **Done**

**Outcome.** `PathSegment` enum (`Named(String)`, `Index(usize)`) and `SelectorPath { parts: Vec<PathSegment> }` struct added to `ast.rs` with `Display` impl and `from_string()` helper. `parse_selector_path()` added to `parser.rs`: consumes `/`-separated `Ident` tokens, handles optional `[N]` index suffixes, validates non-empty path, returns `Result<SelectorPath, ParseError>`. Stub implementations of `parse_replace()`, `parse_insert()`, `parse_delete()` created — each calls `parse_selector_path()` and is routed from `parse_diff()`, eliminating the dead-code warning. `SelectorPath` and `PathSegment` exported in `lib.rs`. 4 integration tests. 195 tests passing, zero warnings. CHANGES REQUESTED after round 1 (orphaned `parse_selector_path`, missing `in_diff_context`); concern 2 fixed (stubs wired up); programmer correctly pushed back on concern 1 (`in_diff_context` deferred to M5.2 — the `/` division ambiguity doesn't arise until diff operation bodies are parsed). APPROVED on round 2. Next: M5.2 — structured `DiffOp` nodes and replace the `Expr::Diff` stub with real grammar.

### M4.6 — Real `@ctx` API (context threading) — **Done**

**Outcome.** `Expr::CtxGet { ctx: Box<Expr>, key: Box<Expr>, node_id: NodeId, span: SourceSpan }` added to `ast.rs`; `Token::CtxGet` added to lexer (keyword `ctx.get`); `parse_ctx_get()` in parser validates that the second argument is a string literal at parse time and errors otherwise; `Env` extended with `context: HashMap<String, Value>` field (initialized empty, propagated via `with_parent()`), `set_context()` and `get_context()` (recursive parent-chain lookup) helpers; eval for `CtxGet` returns the stored value or `Value::Nil` on missing key; typechecker returns `Type::Option(Box<Type::TypeParam("T")>)`; `run_str_with_context(input, HashMap<String, Value>)` added to `lib.rs` as the public API for pre-populating context. 7 integration tests. 191 tests passing (95 unit + 96 integration), zero warnings. CHANGES REQUESTED after round 1 (tests didn't actually set-then-get context values, no non-string-key rejection test); all 3 concerns fixed; APPROVED on round 2.

### M4.5 — Topo-ordered type checking — **Done**

**Outcome.** `topological_sort(dag: &HashMap<ModulePath, Vec<ModulePath>>) -> Result<Vec<ModulePath>, TypeError>` added to `typechecker.rs` using Kahn's algorithm (in-degree counts, BFS queue, self-loops excluded). Called in `typecheck_str()` after `detect_cycles()` for defensive DAG validation. Architecture note: in the single-file AST model, `build_module_caps_map()` pre-populates all module exports before any typechecking runs, so `typecheck(&expr, &env)` on the full AST naturally resolves `@use` statements correctly without per-module iteration — the topo-sort validates ordering correctness, not drives it. 3 new unit tests (`test_topo_sort_single_module`, `test_topo_sort_linear_chain`, `test_topo_sort_diamond`) + 2 integration tests (`test_typecheck_respects_module_order`, `test_use_module_typechecked_first`). 184 tests passing (95 unit + 89 integration), zero warnings. CHANGES REQUESTED after round 1 (reviewer flagged `_sorted_modules` unused); programmer pushed back with valid single-file architecture trace; APPROVED on round 2.

### M4.4 — DAG cycle detection — **Done**

**Outcome.** `build_module_dependency_dag(expr: &Expr) -> HashMap<ModulePath, Vec<ModulePath>>` added to `typechecker.rs`; walks the AST tracking `current_module` to associate each `@use` with its enclosing `@mod`. `detect_cycles(dag) -> Result<(), TypeError>` implements DFS with `visiting`/`visited` sets; self-loops are excluded (a `@use @from foo` inside `@mod foo` is structurally a self-loop in single-file tests and must not be treated as a cycle). `typecheck_str()` calls both after `build_module_caps_map()` and short-circuits on first cycle. Error message includes full cycle path. 4 integration tests. 179 tests passing (92 unit + 87 integration), zero warnings. CHANGES REQUESTED after round 1 (self-loop flag); programmer pushed back with valid trace — self-loop exclusion is required because M4.2 capability tests use `@mod foo / @use @from foo` patterns that structurally create self-loops. Orchestrator accepted pushback; stage closed.

### M4.3 — Dotted module identity — **Done**

**Outcome.** `ModulePath { parts: Vec<String> }` struct added to `ast.rs` with `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash` derives and a `Display` impl (`parts.join("/")`). `Expr::Mod` and `Expr::Use` both changed from `name/module: String` to `name/module: ModulePath`. `parse_mod()` and `parse_use()` in `parser.rs` now consume `/`-separated `Ident` tokens (the lexer's existing `Token::Slash`) and build `ModulePath` structs inline — no lexer changes required. Typechecker's `module_caps: HashMap<String, ...>` re-keyed to `HashMap<ModulePath, ...>`; `build_module_caps_map()` and error messages updated accordingly. 6 new integration tests. 175 tests passing (92 unit + 83 integration), zero warnings. APPROVED on first reviewer pass.

### M4.2 — Capability verification at `@use` — **Done**

**Outcome.** `TypeEnv` in `typechecker.rs` extended with `module_caps: HashMap<String, Vec<CapabilityMarker>>` tracking each module's exported capabilities. New `build_module_caps_map()` function pre-scans the top-level AST for `Expr::Mod`/`Expr::Pub` pairs to populate the map. New `is_cap_subset()` helper performs set containment. The `Expr::Use` typecheck handler now validates: empty caps always pass; non-empty caps verified against the module's exported set; missing module → `TypeError("module not found: ...")`; missing cap → `TypeError("module <name> does not export capability <cap>")`. `typecheck_str()` calls `build_module_caps_map()` before typechecking and populates the initial `TypeEnv`. `module_caps` is cloned into all child envs. 4 integration tests. 169 tests passing (92 unit + 77 integration), zero warnings. APPROVED on first reviewer pass.

### M4.1 — Parse `@mod` and `@pub` declarations — **Done**

**Outcome.** `Mod` and `Pub` tokens added to lexer. `Expr::Mod { name: String, node_id: NodeId, span: SourceSpan }` and `Expr::Pub { cap: Vec<CapabilityMarker>, node_id: NodeId, span: SourceSpan }` added to `ast.rs`. `parse_mod()` and `parse_pub()` added to `parser.rs`, routed from `parse_statement()`; `parse_pub()` uses the existing bracket syntax from `@cap`. Both eval and typechecker return `Nil` (no-op stubs for this stage). 4 integration tests. 165 tests passing (92 unit + 73 integration), zero warnings. APPROVED on first reviewer pass.

### @intent indexing (M3.4) — **Done**

**Outcome.** `IntentEntry { selector, intent_name, subtree_span }` and `IntentTable { entries }` structs added to `ast.rs`. `Parser` extended with `current_path: Vec<String>` and `intent_table: IntentTable`; `push_scope()`/`pop_scope()` helpers track the syntactic scope stack during recursive descent. `parse_fn_def()` and `parse_let()` push/pop their respective names so paths like `/fn greet/let x` are built automatically. `parse_intent()` records each `@intent` node into the table at the current path. `get_intent_table()` drains the table after a parse. `intent_index(source) -> Result<IntentTable, ParseError>` exposed in `lib.rs`. Eval and typechecker unchanged. 5 new unit tests. 161 tests passing, zero warnings. APPROVED on first reviewer pass.

### Typed @uncertain enforcement (M3.3) — **Done**

**Outcome.** `Type::Uncertain(Box<Type>)` added to `ast.rs` as a wrapper variant that tracks uncertainty through the type system. `TypeEnv` in `typechecker.rs` extended with `allow_uncertain: bool` (defaults to `false`); a `with_uncertain_allowed()` constructor creates child envs that permit uncertain values. `Expr::Uncertain` case now typechecks its inner expression in an uncertain-allowed child env and wraps the result in `Type::Uncertain`. Uncertainty is rejected at five boundary sites: `Var` lookup, `Let` binding, `FnDef` return, `Block` let, and `Block` final expression — each emitting `TypeError("uncertain value <name> escapes typed boundary: explicit acknowledgement required")`. `types_compatible()` handles `Type::Uncertain` recursively. `eval.rs` unchanged — uncertainty is purely a typechecker constraint. 5 new integration tests. 156 tests passing, zero warnings. APPROVED on first reviewer pass.

### @err error values (M3.2) — **Done**

**Outcome.** `@ok` and `@err` keywords added to the lexer. `parse_ok()` and `parse_err()` in `parser.rs` parse `@ok expr` / `@err expr` as syntactic sugar, producing `Expr::Tagged("ok", Some(payload), node_id, span)` and `Expr::Tagged("err", ...)`. No changes to `eval.rs` — the M3.1 `Tagged` eval path handles them. Typechecker extended: `Expr::Tagged` now returns `Type::Union([UnionVariant { tag, payload: Some(T) }])` (previously `Type::Symbol`), making tagged values compatible with `@match` scrutinee checks. Effect enforcement: `@err` in a function with `err: false` effect (i.e. `->` arrow) produces a `TypeError`; `@ok` has no effect restriction. 5 integration tests, each exercising both `run_str` and `typecheck_str` paths. 150 tests passing, zero warnings. APPROVED after 2 review rounds (round 1: `Type::Symbol` return fixed to `Type::Union`; `test_ok_err_in_match` extended to call `typecheck_str`; `test_err_effect_type_inference` rewritten with proper effect-function coverage).

### @match control flow (M3.1) — **Done**

**Outcome.** `Expr::Match { scrutinee, patterns, node_id, span }` added to `ast.rs` with a `Pattern` enum (`Tag(String)`, `TagBind(String, String)`, `Wildcard`). `Expr::Tagged(String, Option<Box<Expr>>, NodeId, SourceSpan)` added for `(#tag expr)` constructor syntax. `parse_match()` and `parse_pattern()` added to `parser.rs`; integrated into `parse_statement()`. Eval dispatches by tag: `Tag` matches symbol tags; `TagBind` matches and binds the payload to a child scope variable; `Wildcard` is the fallthrough. Typechecker validates that the scrutinee is a `Union` type, that all union variants are covered (or a wildcard is present), that all branch bodies return the same type, and that `TagBind` pattern variables are bound in a child `TypeEnv` before typechecking the branch body. 7 integration tests. 145 tests passing, zero warnings. APPROVED after 2 review rounds (round 1: missing `test_match_payload_type` + typechecker not binding pattern variables in child env; both fixed).

### Structural types in typechecker (M2.4) — **Done**

**Outcome.** `types_compatible(a: &Type, b: &Type) -> bool` helper added to `typechecker.rs` with recursive structural equality for all `Type` variants: `Primitive` (exact match), `Option`/`List` (recurse on inner), `Record` (field names + types, order-sensitive), `Union` (variant tags + payloads), `Fn` (params + return + effect), `Symbol`, fallback `_ => false`. `FnDef` typechecker path extended: compound return types (`Record`/`Union`/`Option`/`List`) are accepted from annotations without body inference; `types_compatible` used to validate annotated vs inferred return types. 13 new unit tests for `types_compatible` + 5 new integration tests exercising union return types. 138 tests passing, zero warnings. APPROVED on first reviewer pass.

### Orthogonal Effect Set (`EffectSet`) — **Done**

**Outcome.** Replaced linear `EffectLevel` enum with orthogonal `EffectSet` struct (`err: bool`, `io: bool`, `async_: bool`). All 8 arrow symbols now expressible: `->`, `-?>`, `-!>`, `-~>`, `-?!>`, `-?~>`, `-!~>`, `-?!~>`. Lexer: `EffectArrow1/2/3` removed; single `EffectArrow(EffectSet)` token, parsed from `-` + canonical `?!~` flag sequence. Typechecker: level-comparison replaced with `is_subset_of()` — callee effects must be a subset of caller's effects. `TypeEnv.effect_level` upgraded to `Option<EffectSet>`. 5 new integration tests cover all previously-inexpressible combinations. 125 tests passing, zero warnings. APPROVED on first reviewer pass.

### Parse `@use [capabilities] from module` stub — **Done**

**Outcome.** `Use` and `From` tokens added to the lexer. `Expr::Use { caps: Vec<CapabilityMarker>, module: String, node_id: NodeId, span: SourceSpan }` added to the AST. `parse_statement` routes `Token::Use` to new `parse_use()` which parses `@use [ident ...] @from ModuleName` using the same `LeftBracket`/`RightBracket` pattern from `@cap`. Eval returns `Nil` (no-op stub). Typechecker returns `Type::Primitive(Nil)` (not in the plan's files-touched list but necessary for exhaustiveness — accepted deviation). 3 new integration tests. 119 tests passing, zero warnings. APPROVED on first reviewer pass.

### Parse `@cap` markers on `@fn` definitions — **Done**

**Outcome.** `LeftBracket` and `RightBracket` tokens added to the lexer. `Cap` token added for `@cap`. `Expr::FnDef` gained `cap: Vec<CapabilityMarker>` field (between `effect_level` and `node_id`). `parse_fn_def` in `parser.rs` optionally consumes `@cap [ident ...]` after the parameter list and before the effect arrow; defaults to `vec![]` when absent. `eval.rs` ignores the field via `cap: _`. 3 new integration tests. 116 tests passing, zero warnings. APPROVED after 1 review round (bracket syntax fix).

**Workflow note.** Round 1 reviewer correctly flagged that the initial implementation used parentheses `()` instead of square brackets `[]` — violating both the plan and AVEN_SPEC.md Appendix C. Fix required adding `LeftBracket`/`RightBracket` tokens to the lexer.

### Effect enforcement at call sites — **Done**

**Outcome.** `Expr::FnDef` in `ast.rs` gained an `effect_level: EffectLevel` field (defaulting to `Pure`). The parser's `parse_fn_def` now records which effect arrow token was consumed (`->` = Pure, `~>` = Error, `~~>` = Io, `~~~>` = Async). The typechecker's `TypeEnv` gained `effect_level: Option<EffectLevel>` tracking the enclosing function's effect; `typecheck_fn_def` reads the field directly from the AST node, sets child env effect, and `FnCall` handling compares caller vs callee effect and rejects Pure→IO/Async calls with a `TypeError`. Eval ignores the new field via `..`. 3 new integration tests. 113 tests passing, zero warnings. APPROVED after 2 review rounds.

**Workflow note.** Round 1 review correctly identified that the initial implementation's effect-level inference from `return_type` was broken (the return type is `Int`, not `Type::Fn`), and that the three tests were placeholders. Both were fixed: `effect_level` moved onto `Expr::FnDef` directly, and tests were rewritten to exercise real enforcement with AVEN source strings using `~~>` and `->` syntax.

### Add a type-checker skeleton (M2 — bidirectional check) — **Done**

**Outcome.** New `seed/src/typechecker.rs` (272 lines) with `TypeError { span, message }`, `TypeEnv` (parent-chain scope), and `typecheck(expr, env) -> Result<Type, TypeError>`. Handles all required M1 forms (Int/Bool/Str/Nil/Var/Let/FnDef) plus If, Arithmetic, Block, Ret, IoWrite, Symbol, and FnCall/Intent/Ctx/Diff stubs. `typecheck_str` wired into `lib.rs`. 13 new unit tests + 4 new integration tests. `cargo build && cargo test` reports 110 tests passing, zero warnings. APPROVED on first reviewer pass.

**Workflow note.** Block scope chaining was extended beyond the minimal plan to support sequential let bindings within integration tests — a necessary in-scope expansion accepted by the orchestrator.

### Clean unused imports and verify build is warning-free — **Done**

**Outcome.** The unused `SourceSpan` import in `eval.rs` was moved from the top-level `use` statement (line 1) into the `#[cfg(test)]` module, where it is actually used by unit tests. Rust does not count `#[cfg(test)]` usage as satisfying an outer-module import, so the warning was legitimate despite the Programmer initially concluding otherwise. `cargo build` now emits zero warnings; 70 tests pass (45 unit + 25 integration).

**Workflow note.** The Programmer agent misidentified the fix, claiming the import was needed in the outer module. The orchestrator diagnosed the Rust scoping rule (cfg(test) usage doesn't suppress non-test import warnings) and applied the one-line move directly. Reviewer approved. The "clean unused peek methods" entry in the prior roadmap was stale — those methods don't exist in the current codebase.

### Add source spans to AST nodes — **Done**

**Outcome.** Every `Expr` variant except `Nil` carries a `SourceSpan`. The lexer exposes `tokenize_spanned()` / `next_token_spanned()` returning `Vec<(Token, SourceSpan)>`. The parser consumes spanned tokens and attaches subtree-covering spans on every constructed node via a `span_from(start, end)` helper. The evaluator ignores spans (`_` for tuple variants, `..` for struct variants). Five new tests cover spans at lexer / parser / eval layers. `parse_intent` was tightened during review to use the post-`advance()` pattern that the rest of the parsers use (no semantic change, codebase uniformity).

**Workflow note.** Executed as a 3-agent Haiku loop (planner → programmer → reviewer → programmer fix → orchestrator close). Two notable deviations to flag for future runs:

1. **Planner overreach.** The planner violated its role twice: it implemented immediate action #2 (variable lookup, plus `test_var_in_arithmetic` and `test_var_in_nested_arithmetic`) and laid the `SourceSpan` struct + parallel spanned-lexer methods (`next_token_spanned`, `tokenize_spanned`, refactored `next_token` → `next_token_impl`). Both pre-work artifacts were clean and consistent with the plan so the orchestrator accepted them. Next planner run needs a stronger no-code guardrail in the prompt — likely a sentence like "If you find yourself opening a `.rs` file in Write/Edit mode, stop and rewrite the plan."
2. **Reviewer false positive.** Reviewer flagged an off-by-one in `parse_intent`'s span construction. The programmer traced it and confirmed the spans were identical pre- and post-`advance()` (both point at the same string-literal token), but applied the change anyway for style consistency with `parse_let` / `parse_fn_def`. Documented as style-not-bug in the response.

**Build status.** `cargo build` and `cargo test` ran successfully on May 13–14 (build artifacts in `seed/target/debug/`). The span work landed after those runs; re-verification on a real Rust toolchain is the first item in the immediate-next-actions list.

### Add effect arrow tokens to lexer — **Done**

**Outcome.** `EffectArrow1` (`~>`), `EffectArrow2` (`~~>`), and `EffectArrow3` (`~~~>`) are now first-class `Token` variants. The lexer's `next_token_impl()` handles the lookahead chain correctly, including the edge case where `~~~` appears without a trailing `>` (returns `EffectArrow2`, leaves the orphaned `~` for the next token). `parse_fn_def` in `parser.rs` was extended (+12 lines) to accept effect arrows as type-signature terminators alongside `->`, making the integration test feasible without deeper parser work. Four new tests added; 74 tests passing (48 unit + 26 integration), zero warnings.

**Workflow note.** The Programmer made a minor out-of-scope deviation by also modifying `parser.rs`. The plan's "Out of scope" said no parser changes, but the integration test `test_fn_with_effect_arrow_parses` required minimal parser support. The orchestrator accepted the deviation — the change is minimal, correct, and was flagged in the Reviewer's approval. Reviewer verdict: `APPROVED` on the first pass (no review rounds needed).

### Wire Type annotations into `Expr::FnDef` (M2.2) — **Done**

**Outcome.** `Expr::FnDef.params` upgraded from `Vec<(String, Option<String>)>` to `Vec<(String, Option<Type>)>`. `return_type: Option<Type>` added. New `parse_type_expr()` helper parses primitives and union variants. `eval.rs` ignores both fields via `..`. 4 new integration tests. 93 tests passing, zero warnings. APPROVED on first reviewer pass.

### Add type representation to AST (M2 foundation) — **Done**

**Outcome.** `Type` enum added to `ast.rs` covering all M2 type variants: `Primitive(PrimitiveType)`, `Fn { params, return_type, effect, cap }`, `Option`, `List`, `Record`, `Union`, `Symbol`, `TypeParam`, `TypeRef`. Supporting types: `PrimitiveType` (`Int`/`Bool`/`Str`/`Flt`/`Nil`), `EffectLevel` (`Pure`/`Error`/`Io`/`Async`) with `arrow_symbol()` returning `"->"` / `"~>"` / `"~~>"` / `"~~~>"`, `UnionVariant { tag, payload }`, `CapabilityMarker` type alias. Helper methods `Type::is_pure()` and `Type::is_io()`. 11 unit tests added in `ast.rs`. 89 tests passing total. No changes to `parser.rs`, `eval.rs`, or existing `Expr` definitions. APPROVED on first reviewer pass.

### Introduce NodeId infrastructure for M5 @diff engine — **Done**

**Outcome.** `pub type NodeId = u64` added to `ast.rs`. Every `Expr` variant except `Nil` now carries a `node_id: u64` field (positioned before `span`). The `Parser` struct gained a `node_counter: u64` field and a `next_node_id()` method (returns pre-increment value, so the first node parsed gets ID 0). All 23 `Expr` constructor call sites across `parser.rs` were updated. `eval.rs` ignores `node_id` via `_`/`..` patterns with no semantic changes. Four new tests cover ID uniqueness on siblings, nested expressions, and eval invariance. 78 tests passing (52 unit + 26 integration).

**Workflow note.** Two review rounds used. Round 1: Reviewer correctly flagged that `test_nodeid_in_nested_expr` asserted `node_id > 0` for the top-level arithmetic node; the Programmer confirmed `next_node_id()` returns the pre-increment value (first call → ID 0) and applied a fix to `assert_eq!(node_id, 0)`. Round 2: Reviewer correctly re-flagged that the fix was backwards — in a recursive descent parser, leaf nodes are constructed first, so the top-level `+` node is the *last* to receive an ID (not 0). The orchestrator applied the final fix directly: replaced the broken `assert_eq!(node_id, 0)` with assertions that the outer node's ID exceeds both children's IDs, which correctly encodes the parser's bottom-up construction order.

---
- `@use` enforcement — `@use` already parses; this stage does not wire it to module verification.

---

### M5.2 — Structured `DiffOp` nodes — **Done**

**Outcome.** `InsertMode` enum (`First`, `Last`, `Before(String)`, `After(String)`), `DiffKind` enum (`Replace`, `Insert`, `Delete`, `Move`, `Copy`), and `DiffOp` struct (`kind`, `selector`, `payload`, `insert_mode`, `node_id`, `span`) added to `ast.rs`. `Expr::Diff` upgraded from a stub to `Diff { ops: Vec<DiffOp>, node_id, span }`. `parse_replace()`, `parse_insert()`, `parse_delete()`, `parse_move()`, `parse_copy()` fully implemented in `parser.rs`; `parse_diff()` routes to them and collects ops into the struct. Token variants `To`, `First`, `Last`, `Before`, `After` added to lexer. `in_diff_context: bool` flag added to `Parser` struct (set `true` in `parse_diff()`, restored after) as infrastructure for M5.3. `is_expression_start()` helper added; `parse_delete()` rejects trailing expressions. 11 tests (10 plan-required + `test_diff_delete_rejects_payload`). `DiffOp`, `DiffKind`, `InsertMode` exported in `lib.rs`. 206 tests passing (106 unit + 100 integration), zero warnings. CHANGES REQUESTED after round 1 (reviewer mistakenly looked only at integration.rs for tests; concern filtered); concern 4 fixed (`parse_delete` validation); concern 5 fixed (flag added). Round 2 reviewer flagged `in_diff_context` unused — orchestrator closed loop: flag is correct infrastructure for M5.3 when full expression parsing in diff bodies is needed; `/`-vs-division ambiguity has no trigger in M5.2 because payloads use `parse_primary()` which doesn't parse binary ops.

### M5.3 — @meta blocks and diff metadata — **Done**

**Outcome.** `DiffMetadata { description: Option<String>, author: Option<String>, timestamp: Option<String> }` struct added to `ast.rs`. `Expr::Diff` gained `metadata: Option<DiffMetadata>` field (before `node_id`). `parse_meta()` added to `parser.rs` consuming `@meta { key: "value" ... }` blocks; `parse_diff()` calls it on first token and stores result or `None`. `Token::Colon` added to lexer to support `key: value` syntax (in-scope deviation — required for the key-value format). `DiffMetadata` exported in `lib.rs`. Tests: `test_meta_description_only`, `test_meta_all_fields`, `test_meta_missing_optional_ok`. 209 tests passing (106 unit + 103 integration), zero warnings. APPROVED on first reviewer pass.

### M5.1 — Selector path parser — **Done**

**Outcome.** `PathSegment` enum (`Named(String)`, `Index(usize)`) and `SelectorPath { parts: Vec<PathSegment> }` struct added to `ast.rs` with `Display` impl and `from_string()` helper. `parse_selector_path()` added to `parser.rs`: consumes `/`-separated `Ident` tokens, handles optional `[N]` index suffixes, validates non-empty path, returns `Result<SelectorPath, ParseError>`. Stub implementations of `parse_replace()`, `parse_insert()`, `parse_delete()` created in `parser.rs` — each calls `parse_selector_path()` and is routed from the `parse_diff()` match, eliminating the dead-code warning. `SelectorPath` and `PathSegment` exported in `lib.rs`. 4 integration tests. 195 tests passing, zero warnings. CHANGES REQUESTED after round 1 (orphaned `parse_selector_path`, missing `in_diff_context` flag); concern 2 fixed (stubs wired up); programmer correctly pushed back on concern 1 (`in_diff_context` deferred to M5.2 — the `/` division ambiguity doesn't arise until diff operation bodies are parsed); APPROVED on round 2.

### M6.5 — aven/std/time stdlib module — **Done**

**Outcome.** `chrono = { version = "0.4", features = ["std"] }` added to `Cargo.toml`. `now` (arity 0), `sleep` (arity 1, ms as Int), and `format` (arity 2, Int timestamp + Str format string) registered in `register_stdlib()` under both `aven/std/time::*` qualified names and `time_*` short aliases — six closures total, two separate per function (pre-accepted pattern). `now` returns current Unix seconds via `SystemTime`. `sleep` rejects negative ms with `InvalidOperation`. `format` uses `chrono::Utc.timestamp_opt(...).single()` for safe timestamp construction, then wraps `dt.format(fmt).to_string()` in `std::panic::catch_unwind(AssertUnwindSafe(...))` to convert chrono's format-string panic into `EvalError::InvalidOperation("invalid format string")` — critical fix from Opus Round 1. 18 integration tests. 344 tests passing (142 unit + 202 integration), zero warnings. Opus approved Round 2.

**Workflow notes.** Haiku reviewer Round 1 caught missing `test_time_sleep_positive_millis`; fixed. Opus Round 1 found format-string panic defect (not covered by any test) — fixed with `catch_unwind`; also added `test_time_format_invalid_format_string`, `test_time_now_arity_one`, `test_time_now_qualified_execution`. Opus Round 2 approved; noted that the default panic hook still prints to stderr before `catch_unwind` swallows the unwind (cosmetic, non-blocking).

---

### M5.8 — `.avenpatch` file serialization — **Done**

**Goal.** Agents compose patch sets offline as serialized `.avenpatch` files (AVEN text format), then apply them to a module at runtime. This completes the M5 `@diff` engine by adding the last M5 item: structured patch file I/O. Spec §2.8 defines the format: `@patch-for path:"<file>" @diff ... @diff ...`. Implements round-trip serialization: `Expr::Diff` → `.avenpatch` text + `.avenpatch` text → `Expr::Diff` (via parser). Integrates with existing `apply_diff`/`diffs_apply` infrastructure so patches can be loaded from files and applied directly.

**Files touched:**
- `/Users/roee.ashkenazi/Desktop/AVEN/seed/src/parser.rs` — add `parse_patch_file()` entry point
- `/Users/roee.ashkenazi/Desktop/AVEN/seed/src/lib.rs` — export `patch_file_to_diffs()` and `diffs_to_avenpatch_string()` helpers
- `/Users/roee.ashkenazi/Desktop/AVEN/seed/tests/integration.rs` — round-trip tests

**Specific changes:**
- `parser.rs`: Add `parse_patch_file()` public method consuming the `@patch-for path:"<file>" @diff ... @diff ...` grammar; routes to existing `parse_diff()` for each batch. Reuse `parse_diff()` entirely — no new parsing logic required since `@diff`/`@diffs` structure is already complete (M5.2+).
- `lib.rs`: Export `patch_file_to_diffs(text: &str) -> Result<Vec<DiffOp>, ParseError>` wrapper around `parse_patch_file()`. Export `diffs_to_avenpatch_string(ops: &[DiffOp], target_path: &str) -> String` serializer — format each op as `@<kind> <selector> [payload]` + collect into a `@patch-for path:"..." @diff ... @diff ...` block.
- `tests/integration.rs`: Five tests: `test_parse_patch_file_single_diff`, `test_parse_patch_file_multiple_diffs`, `test_patch_file_roundtrip_replace`, `test_patch_file_roundtrip_insert`, `test_patch_file_serialization_preserves_semantics` (load file → apply to AST → check result matches expected AST).

**Tests to add:**
1. `test_parse_patch_file_single_diff` — parse `@patch-for path:"src/module.av" @diff @replace /fn x (+ 1 2)` and extract the DiffOp.
2. `test_parse_patch_file_multiple_diffs` — parse a `@patch-for` block with two `@diff` statements; verify both ops appear in output vector.
3. `test_patch_file_roundtrip_replace` — serialize a Replace DiffOp to `.avenpatch` format, re-parse, verify equality.
4. `test_patch_file_roundtrip_insert` — serialize Insert ops with all four InsertMode variants; re-parse and verify.
5. `test_patch_file_serialization_preserves_semantics` — load a `.avenpatch` patch file, apply to a sample AST, verify the resulting AST is type-correct (uses `typecheck_str`).

**Definition of done:**
- `cargo build` and `cargo test` pass (255+ tests expected).
- `patch_file_to_diffs()` and `diffs_to_avenpatch_string()` exported in `lib.rs`.
- `.avenpatch` files can be read and written without loss of semantic information.
- All five tests pass; round-trip preserves DiffOp structure exactly.
- Zero warnings.

**Out of scope:**
- Filesystem I/O (reading/writing `.avenpatch` files to disk — that's Stage 2 tooling).
- Multi-file patch semantics (one `.avenpatch` file per target path; cross-file coordination deferred to M6+).
- Patch composition UI or merge conflict resolution.

**Outcome.** `parse_patch_file()` added to `parser.rs` consuming `@patch-for path:"<file>" @diff ... @diff ...` grammar, reusing existing `parse_diff()` infrastructure. `patch_file_to_diffs(text) -> Result<Vec<DiffOp>, ParseError>` and `diffs_to_avenpatch_string(ops, target_path) -> String` exported from `lib.rs`. Serializer handles all five DiffKinds: Replace/Insert/Delete emit AVEN syntax; Move/Copy pair consecutive same-kind ops as `@move /src @to /dst`. `expr_to_string` handles Int/Bool/Str (with escape)/Symbol/Nil/Arithmetic (correct `+`/`-`/`*`/`/` tokens)/Block. 7 new integration tests (5 plan-required + 2 Move/Copy roundtrip). 239 tests passing (123 unit + 116 integration), zero warnings. 3 Opus rounds: Round 1 found Move/Copy fundamentally broken + ArithOp debug format + string escaping + silent break/drop; Round 2 found selector preservation missing from Move/Copy roundtrip tests; Round 3 approved.

---

### POC Item 2 — aven verify subcommand — **Done**

**Outcome.** `verify` subcommand added to `seed/src/main.rs`. Routing: `if args[1] == "verify"` before REPL fallback. `run_verify()` runs parse → typecheck → check_uncertainty in sequence; first failure exits 1 with JSON to stdout; success exits 0 with JSON to stdout (pure) and `OK: <file>` to stderr. `escape_json_string()` helper handles all control chars 0x00–0x1F. Uncertainty violation message: `"uncertain annotations at: /path1, /path2"` (leading slash, plain string, no embedded JSON). 10 tests: 5 library (parse_str/typecheck_str/check_uncertainty directly) + 5 binary spawn tests using `env!("CARGO_BIN_EXE_aven")` with PID-suffixed temp files. 445 tests passing (436→445), zero warnings. 3 Opus rounds: R1 found invalid uncertainty JSON + stdout/JSON conflict + missing uncertainty binary test; R2 found missing PID suffix + missing typecheck/uncertainty binary spawn tests; R3 approved.

---

## Typechecker hardening (code review backlog)

| ID | Priority | Task |
|----|----------|------|
| TC-R01 | P0 | ~~Apply `topological_sort` in `typecheck_str`~~ **Done (M4.5-fix)** |
| TC-R02 | P0 | ~~`FnCall`: arity + argument types vs callee `params`~~ **Done** |
| TC-R03 | P0 | ~~Validate compound return annotations against function body~~ **Done** |

### Review follow-ups (2026-05-21) — queued after M6

| ID | Priority | Target | Acceptance criteria |
|----|----------|--------|---------------------|
| TC-R04 | P1 | **Distinct unannotated-param marker** | Do not use `Primitive(Nil)` as sentinel for missing param types; skip or check args without conflating explicit `Nil` annotations. Test: `@fn f :: x:Nil -> Int` + wrong arg type fails. |
| TC-R05 | P1 | **Cross-module function namespace** | Detect duplicate `FnDef` names across modules (error) or introduce qualified `@call` / module-scoped exports (document chosen design). Test: two modules define same fn name. |
| TC-R06 | P2 | **Topo-order integration test** | Restore/strengthen `test_topo_module_order_reversed_source_ok` in `integration.rs`; tighten `test_typecheck_respects_module_order` so it fails without reordering (cross-module `@call`). |
| TC-R07 | P2 | **`@uncertain` on ordered path** | `typecheck_program_ordered` applies same final boundary check as `typecheck_block_stmts` for uncertain escape. |
| TC-R08 | P2 | **Recursive function typing** | Provisional self-type in `FnDef` uses annotated `return_type` (or fixpoint), not `Nil`. Test: recursive `Int -> Int`. |
| TC-R09 | P2 | **`@match` branch compatibility** | `merge_branch_type` uses `types_compatible`, not `==`. Test: structurally equal compound branch types. |
| TC-R10 | P3 | **Prelude / root `@call` effects** | Document or enforce effect policy when `TypeEnv.effect_level` is `None` (script/REPL top level). |
| TC-R11 | P3 | **`partition_by_module` completeness** | Include `@mod` nodes in per-module stmt lists or document omission; deep-scan nested `@mod` if in scope. |
| TC-R12 | P3 | **`types_compatible` on `Fn` effects** | Align function-type equality with call-site `is_subset_of` semantics (or document intentional strictness). |
| TC-R13 | P3 | **REPL vs typechecker** | `main.rs` help text notes eval-only REPL; optional `typecheck_str` before eval. |

---

### Compound type inference (§1.7) — **Done**

**Outcome.** `Expr::Record { fields: Vec<(String, Expr)>, node_id, span }` added to AST; `parse_record()` in parser handles `{key:val, ...}` syntax (comma-separated, trailing comma ok, empty records rejected, duplicate field names rejected with ParseError). `Value::Record(Vec<(String, Value)>)` added to eval with Debug/Display/PartialEq. `infer_record_type()` helper in typechecker returns `Type::Record` from field expression types. `Expr::Record` arm added to `fmt.rs::format_expr` emitting `{key:val, ...}` (type formatter keeps `"<type>"` — parse_type_expr can't round-trip record types). 9 integration tests + 1 unit test. 442 tests passing (167 unit + 275 integration), zero warnings. Opus approved Round 2.

**Workflow notes.** Haiku reviewer found 2 missing tests (test_record_infer_wrong_field_type, test_record_annotated_overrides_inference); programmer added them. Opus Round 1 found fmt.rs formatter regression (Record → "<expr>") and missing duplicate-field detection; both fixed. Round 2 approved; noted lib.rs expr_to_string also missing Record arm — pre-existing serializer gap, filtered out-of-scope.

---

---

## Immediate next actions (queued)

32. ~~**M4.5-fix — Apply topo-ordered module typechecking (TC-R01).**~~ **Done.** `partition_by_module`, `typecheck_program_ordered`, `sorted_modules` wired in `typecheck_str`. Tests: `test_typecheck_reversed_module_order_in_source`, `test_topo_module_order_reversed_source_ok`. 233 tests passing.
33. ~~**`FnCall` arity and argument type checking (TC-R02).** Match callee `params` at call sites.~~ **Done.** Tests: `test_fncall_arity_mismatch_too_few`, `test_fncall_arity_mismatch_too_many`, `test_fncall_arg_type_mismatch`, `test_fncall_arg_type_correct`, `test_fncall_unannotated_params`, `test_fncall_non_function_call`, `test_fncall_zero_param_correct`, `test_fncall_zero_param_arity_mismatch`, `test_fncall_arg_type_mismatch_second_arg`. 253 tests passing.
34. ~~**Compound return type validation (TC-R03).** Union/record/option/list return annotations validated against body.~~ **Done.** Tests: `test_fn_union_return_type_mismatch`, `test_fn_record_return_type_mismatch`, `test_fn_option_return_type_mismatch`, `test_fn_list_return_type_mismatch`, `test_fn_compound_return_type_correct`, `test_fn_nested_compound_return_type`, `test_fn_union_variant_acceptance_ok`, `test_fn_union_variant_acceptance_multi`, `test_fn_union_two_variants_acceptance`. 261 tests passing.
35. ~~**M6.1 — NativeFn value type + `aven/std/math` stdlib module.**~~ **Done.** `Value::NativeFn` added to eval; `abs`, `min`, `max`, `pow`, `sqrt` registered under both `aven/std/math::*` and short aliases. Tests: `test_native_fn_abs`, `test_native_fn_min`, `test_native_fn_max`, `test_native_fn_pow`, `test_native_fn_sqrt`, error-path tests, module-qualified lookup tests, Display format test. 281 tests passing.
36. ~~**M6.2 — `aven/std/io` stdlib module.**~~ **Done.**
37. ~~**M6.3 — `aven/std/fs` stdlib module.**~~ **Done.** `read_file_from` helper extracted; `read`, `write`, `list` registered under `aven/std/fs::*` and `fs_read`/`fs_write`/`fs_list` aliases. Tests: `test_fs_read_file`, `test_fs_read_not_found`, `test_fs_read_wrong_type`, `test_fs_write_file`, `test_fs_write_wrong_type`, `test_fs_list_dir`, `test_fs_list_not_found`, `test_fs_list_wrong_type`, `test_fs_qualified_lookup`, `test_fs_short_alias_lookup`, `test_fs_read_on_directory`, `test_fs_list_on_regular_file`, `test_fs_write_then_read`, `test_fs_list_sorted`, `test_fs_list_empty_dir`. 306 tests passing (142 unit + 164 integration).
38. ~~**M6.4 — `aven/std/json` stdlib module.**~~ **Done.** `json_parse` and `json_serialize` registered under `aven/std/json::*` and short aliases. Tests: `test_json_parse_null`, `test_json_parse_bool_true`, `test_json_parse_bool_false`, `test_json_parse_integer`, `test_json_parse_negative_integer`, `test_json_parse_string`, `test_json_parse_array`, `test_json_parse_invalid_json`, `test_json_parse_float_rejected`, `test_json_parse_object_rejected`, `test_json_serialize_nil`, `test_json_serialize_bool`, `test_json_serialize_int`, `test_json_serialize_string`, `test_json_serialize_nativefn_error`, `test_json_roundtrip_int_parse_serialize`, `test_json_roundtrip_string_parse_serialize`, `test_json_parse_qualified_name_lookup`, `test_json_parse_short_alias_lookup`, `test_json_parse_wrong_argument_type`, plus 13 Opus-fix tests. 326 tests passing (142 unit + 184 integration).
39. ~~**M6.5 — `aven/std/time` stdlib module.**~~ **Done.** `now`, `sleep`, `format` registered under `aven/std/time::*` and `time_*` aliases. `chrono = "0.4"` added. `format` uses `catch_unwind(AssertUnwindSafe(...))` to trap panic on malformed format strings, returning `InvalidOperation`. Tests: `test_time_now_returns_int`, `test_time_now_qualified_name`, `test_time_now_arity_one`, `test_time_now_qualified_execution`, `test_time_sleep_zero_millis`, `test_time_sleep_positive_millis`, `test_time_sleep_negative_millis`, `test_time_sleep_wrong_type`, `test_time_sleep_qualified_name`, `test_time_sleep_arity_zero`, `test_time_sleep_arity_two`, `test_time_format_valid_timestamp`, `test_time_format_different_format`, `test_time_format_qualified_name`, `test_time_format_wrong_type_timestamp`, `test_time_format_wrong_type_format`, `test_time_format_roundtrip`, `test_time_format_invalid_format_string`. 344 tests passing (142 unit + 202 integration).
40. ~~**M6.6 — `aven/std/collections` stdlib module.**~~ **Done.** `list`, `map`, `set` registered under `aven/std/collections::*` and `col_list`/`col_map`/`col_set` aliases. 19 integration tests. 363 tests passing (142 unit + 221 integration).

---

### M6.6 — aven/std/collections stdlib module — **Done**

**Outcome.** `list` (arity 1), `map` (arity 2), `set` (arity 1) registered in `register_stdlib()` under both `aven/std/collections::*` qualified names and `col_list`/`col_map`/`col_set` short aliases — six closures total. All operate on the `\n`-joined Str convention: `list` normalizes (splits, filters empty, rejoins), `map` applies a NativeFn to each element (requires Str return, errors with TypeError otherwise), `set` deduplicates and sorts via `BTreeSet`. `use std::collections::BTreeSet;` placed inside `register_stdlib()` at function scope. 19 integration tests. 363 tests passing (142 unit + 221 integration), zero warnings. Opus approved Round 1.

**Workflow notes.** Opus found two non-blocking diagnostics observations: the `(NativeFn, non-Str)` map arm falls through to the catch-all `_ =>` with a slightly misleading error message ("requires NativeFn and Str" when only arg1 is wrong), and `test_col_map_wrong_type_list` only asserts `is_err()` rather than the specific message. Neither blocks correctness; Opus marked both non-blocking and approved.

41. ~~**M6.7 — `aven/std/http` stdlib module.**~~ **Done.** `get`, `post`, `put`, `delete` registered under `aven/std/http::*` and `http_*` aliases. `ureq = "2"` added. All error paths: TypeError for wrong arg types, InvalidOperation with status code for non-2xx, InvalidOperation via transport arm for invalid URLs. 23 integration tests (4 `#[ignore]` live). 382 tests passing (142 unit + 240 integration, 4 ignored). Opus approved Round 2.

42. ~~**TC-R04 — Distinct unannotated-param marker.**~~ **Done.** See Completed Stages.
43. ~~**TC-R05 — Cross-module function namespace.**~~ **Done.** See Completed Stages.
44. ~~**TC-R06 — Topo-order integration test.**~~ **Done.** See Completed Stages.
45. ~~**TC-R07 — `@uncertain` boundary on ordered typecheck path.**~~ **Done.** See Completed Stages.
46. ~~**TC-R08 — Recursive function return typing.**~~ **Done.** See Completed Stages.
47. ~~**TC-R09 — `@match` branches via `types_compatible`.**~~ **Done.** See Completed Stages.
48. ~~**TC-R10 — Prelude / root `@call` effect policy.**~~ **Done.** See Completed Stages.
49. ~~**TC-R11 — `partition_by_module` / deep module scan.**~~ **Done.** See Completed Stages.
50. ~~**TC-R12 — `types_compatible` Fn effect alignment.** Subset vs exact equality for `Type::Fn` effects.~~ **Done.** Tests: `test_types_compatible_fn_pure_found_io_expected`, `test_types_compatible_fn_io_found_pure_expected`, `test_types_compatible_fn_same_effects`. 360 tests passing.
51. ~~**TC-R13 — REPL typecheck UX.** Document eval-only REPL; optional typecheck before eval in `main.rs`.~~ **Done.** Tests: `test_repl_eval_without_typecheck_allows_type_mismatch`, `test_repl_typecheck_mode_rejects_before_eval`, `test_repl_typecheck_stateless_per_line`. 363 tests passing.

52. ~~**M7.1 — Span-aware error messages.**~~ **Done.** Tests: `test_error_message_includes_line_col_var_undefined`, `test_error_message_includes_line_col_multiline`, `test_error_message_zero_span_omits_prefix`, `test_error_message_offset_zero_span`. 367 tests passing.

60. ~~**Module resolution algorithm (§3.9) narrative test.** Write integration test covering 7 steps: collect @mod, assign identity, collect @use, verify capability subset, detect cycles, topo-sort, type-check in order.~~ **Done.** Test: `test_module_resolution_7step_narrative` (exercises all 7 steps with two-module program). 443 tests passing.

61. ~~**M7 — `aven repl` (multi-line input, history, prompt continuation).**~~ **Done.** `run_repl()` with paren-balance tracking (string-literal-aware, comment-aware, negative-depth recovery), `aven> ` / `...> ` prompts, stdlib preload. `run_str_with_env` added to lib.rs. 3 integration tests. 436 tests total. Opus approved Round 3.

62. ~~**M7 — `@intent` index dump (`aven intent <module>`).**~~ **Done as item 71.** `run_intent()` + `format_intent_output()` implemented. 5 integration tests. 388 tests passing at close.

63. ~~**M6 — `aven/std/json` stdlib module.**~~ **Done.** `json_parse`/`json_serialize` NativeFn closures registered under `aven/std/json::parse`/`aven/std/json::serialize` and short aliases. Uses `serde_json`. 24 JSON tests passing. 396 tests total.

64. ~~**M6 — `aven/std/time` stdlib module.**~~ **Done.** `time_now`/`time_sleep`/`time_format` NativeFns registered under `aven/std/time::*` and short aliases. Uses Rust std + chrono. 10 integration tests. 396 tests total.

65. ~~**M6 — `aven/std/math` stdlib module.**~~ **Done.** add/sub/mul/div/floor/ceil/round added as `checked_*` NativeFns under `aven/std/math::*` and short aliases. pow also fixed to use `checked_pow`. 17 new tests (qualified names, overflow paths, error paths). 414 tests total. Opus approved Round 3.

66. ~~**M6 — `aven/std/collections` stdlib module.**~~ **Done.** Value::Map and Value::Set added; 12 NativeFns (+ 12 aliases) registered under `aven/std/collections::*` and `col_*`. Order-independent Map equality; set_add rejects Fn/NaN. 19 new integration tests. 433 tests total. Opus approved Round 2.

---


67. ~~**M5.3 — `@meta` blocks.**~~ **Done.** `DiffMetadata` struct with description/author/timestamp fields added to `ast.rs`; `parse_meta()` parses `@meta { key: "value" ... }` blocks; `Expr::Diff` gained `metadata: Option<DiffMetadata>` field. Tests: `test_meta_description_only`, `test_meta_all_fields`, `test_meta_missing_optional_ok`. 209 tests passing at completion.

68. ~~**M5.8 — `.avenpatch` file serialization.**~~ **Done.** `parse_patch_file()` in `parser.rs` consumes `@patch-for path:"<file>" @diff ... @diff ...` grammar. `patch_file_to_diffs()` and `diffs_to_avenpatch_string()` exported from `lib.rs`. Round-trip serialization of `Expr::Diff` ↔ `.avenpatch` text. 7 integration tests. 239 tests passing at completion.

69. ~~**M7 — `aven repl` (multi-line input, history, prompt continuation).**~~ **Done as item 61.** Duplicate entry — closed with item 61.

---

### Module resolution algorithm (§3.9) narrative test — **Done**

**Outcome.** Confirmed the comprehensive integration test `test_module_resolution_7step_narrative` already exists at `tests/integration.rs:3406` and passes. It constructs a two-module AVEN program (`math_helpers`, `app`) with `@pub [calc]` / `@use [calc] @from math_helpers`, then exercises all 7 steps of §3.9 in sequence: `parse_str` → `build_module_caps_map` (collect+assign) → `build_module_dependency_dag` (collect+edges) → `detect_cycles` → `topological_sort` → `typecheck_program_ordered`. Each step's output is asserted explicitly: caps map contains both modules with correct exports, DAG has app→math_helpers edge, no cycle, topo-sort produces `[math_helpers, app]`, topo-ordered typecheck returns `Ok`. 443 tests passing (167 unit + 276 integration), zero warnings. No code changes required — the stage was already implemented in a prior cycle; this entry closes the ROADMAP tracking gap.

---

### Symbol type (#-prefix) — lexer + parser + eval + typecheck — **Done**

**Outcome.** `#<name>` symbol literals now fully supported per §1.2. New `read_symbol_name()` helper in `lexer.rs` reads only `[a-zA-Z_][a-zA-Z0-9_]*` after `#`, stored as `Token::Ident("#foo")`; `parse_primary()` routes hash-prefixed idents to `Expr::Symbol`; validation rejects bare `#`, digit-leading names, and `#`-prefixed record field names. Dead `Token::Hash` removed from `is_expression_start()`. 9 integration tests: `test_symbol_hash_prefix_parses`, `test_symbol_hash_prefix_evals`, `test_symbol_hash_prefix_typechecks`, `test_symbol_hash_equality`, `test_symbol_hash_in_record_field`, `test_symbol_hash_invalid_no_name`, `test_symbol_hash_digit_leading_rejected`, `test_symbol_hash_dotted_stops_at_name`, `test_symbol_hash_as_record_key_rejected`. 488 tests passing (167 unit + 321 integration), zero warnings. Opus approved Round 2.

**Workflow notes.** Opus Round 1 found 8 issues; most were real. `read_ident()` was replaced with `read_symbol_name()` to enforce proper identifier constraints. Concerns 1-2 (AVEN `==` operator on symbols) correctly pushed back — AVEN M1 has no `==` operator; Rust PartialEq comparison is the right test approach. Symbol/Tagged/Union `#name` collision (concern 6) deferred as pre-existing design.

---

### M7.1 — Span-aware error messages — **Done**

**Outcome.** `source_to_line_col(source: &str, byte_offset: usize) -> (usize, usize)` added to `lib.rs`; `TypeError::display_with_source(source: &str) -> String` added to `typechecker.rs` — emits `"{line}:{col}: {message}"` when `self.span.end > 0` (using `end > 0` as the sentinel, not `start > 0`, to correctly handle errors at byte offset 0). 4 integration tests: `test_error_message_includes_line_col_var_undefined`, `test_error_message_includes_line_col_multiline`, `test_error_message_zero_span_omits_prefix`, `test_error_message_offset_zero_span` (added in Opus Round 2). 367 tests passing (150 unit + 217 integration), zero warnings. Opus approved Round 2.

**Workflow notes.** Opus Round 1 found 3 defects: (1) `start > 0` sentinel conflated real offset-0 errors with the zero sentinel — fixed to `end > 0`; (2) no test covered offset-0 boundary — added `test_error_message_offset_zero_span`; (3) vacuous `contains(":")` assertion — replaced with `starts_with("1:")`. Opus Round 2 approved all fixes.

---

