# AVEN Compiler — Rust Implementation Plan

## Compiler Architecture

### Crate Design

Single-binary, well-modularized crate (multi-crate overhead isn't justified for a seed):

```
aven/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI entrypoint
│   ├── driver.rs            # Compilation pipeline orchestration
│   ├── syntax/
│   │   ├── mod.rs
│   │   ├── span.rs          # SourceLocation, ByteSpan
│   │   ├── token.rs         # Token enum
│   │   └── ast.rs           # AST node definitions + NodeId
│   ├── lexer.rs             # String → Vec<Token>
│   ├── parser.rs            # Tokens → AST (recursive descent, no lookahead)
│   ├── types.rs             # Type representations, unification
│   ├── check.rs             # Type checker + effect + capability inference
│   ├── module.rs            # Module graph, resolution, cyclic dep detection
│   ├── diff/
│   │   ├── mod.rs
│   │   ├── selector.rs      # Selector path parsing (/fn greet/body/ret)
│   │   └── patch.rs         # Patch application engine
│   └── eval.rs              # AST-walking interpreter (seed runtime)
├── tests/
│   ├── fixtures/            # .aven test files
│   ├── lexer_tests.rs
│   ├── parser_tests.rs
│   ├── typecheck_tests.rs
│   ├── eval_tests.rs
│   └── diff_tests.rs
└── examples/                # Example .aven programs
```

### Data Flow

```
Source text
    → Lexer (String → [Token with spans])
    → Parser ([Token] → AST with NodeIds)
    → Module resolver (resolve imports, build module graph)
    → Type checker (assign types, verify capabilities, track effects)
    → (optional) @diff engine (apply patches to AST)
    → Evaluator (walk AST, produce value)
    → Output (stdout, file, or value)
```

### Core Data Structures

**Span:**
```rust
struct SourceSpan { start: usize, end: usize }
```
- Every token and AST node carries a span for error reporting.

**Token:**
```rust
enum Token {
    At,              // @
    // Note: Hash (#) is never emitted standalone — the lexer produces SymbolLit directly
    Colon,           // :
    DoubleColon,     // ::
    Arrow,           // ->
    TildeArrow,      // ~>
    DoubleTildeArrow, // ~~>
    TripleTildeArrow, // ~~~>
    OpenBrace,       // {
    CloseBrace,      // }
    OpenBracket,     // [
    CloseBracket,    // ]
    Pipe,            // |
    Underscore,      // _
    Plus,            // +
    Minus,           // -
    Star,            // *
    Slash,           // /
    EqEq,            // ==
    NotEq,           // !=
    Lt, Gt, LtEq, GtEq, // < > <= >=
    Equals,          // =
    Dot,             // .
    // No Comma — AVEN is whitespace-delimited; commas are not part of the syntax
    LParen, RParen,  // ( )
    Ident(String),
    StrLit(String),
    IntLit(i128),
    FltLit(f64),
    SymbolLit(String), // #foo — lexed as a single token, Hash never appears alone
    Indent,           // indent level increase
    Dedent,           // indent level decrease
    Newline,
    Eof,
}
```

**NodeId — key design decision for patch-friendliness:**
```rust
struct NodeId(u64);
```
Every node in the AST gets a unique, stable ID. Selector paths can address by ID directly (`/42`) or by name path with disambiguation (`/fn greet/body/let[0]`). This is what makes `@diff` possible without text-based line matching.

**AST — representative node types:**
```rust
struct ModDecl { id: NodeId, name: Option<Ident>, decls: Vec<Decl> }
struct UseDecl { id: NodeId, bindings: Vec<UseBinding>, source: Path }
struct FnDecl { id: NodeId, name: Ident, params: Vec<Param>, ret: Option<TypeExpr>, effect: Effect, body: Expr }
struct LetDecl { id: NodeId, name: Ident, ty: Option<TypeExpr>, body: Expr } // body is always required
struct TypeDecl { id: NodeId, name: Ident, params: Vec<Ident>, ty: TypeExpr }

enum Expr {
    Lit(NodeId, Literal),
    Ident(NodeId, String),
    Call(NodeId, Box<Expr>, Vec<Expr>),
    BinOp(NodeId, BinOp, Box<Expr>, Box<Expr>),
    If(NodeId, Box<Expr>, Box<Expr>, Box<Expr>),
    Match(NodeId, Box<Expr>, Vec<Arm>),
    Block(NodeId, Vec<LetDecl>, Box<Expr>), // sequence of let bindings, final expr is the block value
    Record(NodeId, Vec<(Ident, Expr)>),
    Field(NodeId, Box<Expr>, Ident),
    Err(NodeId, Box<Expr>),
    Async(NodeId, Box<Expr>),
    Await(NodeId, Box<Expr>),
    Ctx(NodeId),
    Uncertain(NodeId, Box<Expr>),
}
```

**Type representation:**
```rust
enum Type {
    Prim(Primitive),
    Record(Vec<(Ident, Type)>, bool),  // bool = closed (nominal) vs open (structural)
    Union(Vec<Variant>),
    List(Box<Type>),
    Option_(Box<Type>),
    Fun(Box<Type>, Box<Type>, Effect),
    Ref(Path),
    Var(TypeVar),          // inference variable
    Generic(Ident),
    Cap(Ident),
    Never,
}

struct TypeVar(u64);  // unification variable with union-find
```

### Lexer Design

**Key challenge: indentation tracking.**

Approach: line-based processing that emits `Indent`/`Dedent` tokens.
- Maintain an indentation stack (Vec<usize>).
- Each logical line: count leading whitespace, compare to stack top.
- If greater → `Indent` + push.
- If lesser → pop until match, emitting `Dedent` for each level.
  - If no match → indentation error.

Indentation tokens appear *between* other tokens, not as their own lines. The parser sees a flat sequence: `Ident @fn Ident :: Ident : Ident Arrow Ident Newline Indent @ret StrLit Newline Dedent Newline`.

Comments (`;`) are stripped in the lexer — they go to end of line and are discarded. Blank lines are skipped.

**No string escapes** beyond `\"` and `\\` — keep it simple.

### Parser Design

**Recursive descent, one token lookahead.**

Because every expression starts with `@intent` — the parser is trivially unambiguous. The first token tells you what you're parsing:

| Token | Parse starts |
|---|---|
| `@fn` | function declaration |
| `@let` | let binding |
| `@type` | type alias |
| `@use` | import |
| `@mod` | submodule |
| `@pub` | public marker |
| `@if` | if expression |
| `@match` | match expression |
| `@ret` | return expression |
| `@call` | function call |
| `@err` | error expression |
| `@async` | async block |
| `@await` | await expression |
| `@ctx` | context access |
| `@uncertain` | uncertain block |
| `@diff` / `@diffs` | patch format |
| `@intent` | doc intent string |
| `@io` | IO operation |
| `@true` / `@false` | boolean literals |
| Ident / `{` / `[` / `(` / `#` / `_` / `"` / digit | expression |

**Indentation-based blocks:** After parsing the header of a block-introducing node (e.g., `@fn greet :: ... -> Str`), the parser expects either `Newline Indent` (block body follows) or an inline expression (for simple single-line functions).

**No operator precedence:** Every binary expression is `atom op atom` — exactly two non-binary-op operands. Chaining or mixing operators requires explicit parentheses. This is the Smalltalk model:
- `a + b` — valid (two atoms)
- `a + b + c` — **invalid**; must write `(a + b) + c`
- `a + b * c` — **invalid**; must write `(a + b) * c` or `a + (b * c)`

The parser enforces this by: after successfully parsing the right-hand side of a binary op, if the next token is another binary operator, it emits a parse error: "operator chaining requires explicit parentheses." This means `@ret "Hello, " + name` is valid (two atoms), but `@ret a + b + c` is a parse error.

### Type Checker Design

**Algorithm: bidirectional type checking** (not full Hindley-Milner).

- **Input types** are checked (known type → expression)
- **Output types** are inferred (expression → synthesize type)
- Local `@let` without annotation infers from the value expression
- Function return type without annotation infers from body

**Effect checking:** Each function has an effect level. The body is checked against that level:
- If fn is `->`, body cannot contain `@call` to `~>` functions or `@io.*` or `@err`
- If fn is `~>`, body can contain `@err` but not `@io.*`
- If fn is `~~>`, body can contain IO but not `@async`/`@await`
- Effect levels form a lattice: `Pure < Err < IO < Async`

**Capability checking:**
- `@use [read write] from fs` brings capabilities `read` and `write` into scope
- A function that uses file I/O must have capability annotations or be in a module that carries them
- Capabilities are checked at module boundaries

**Type errors:**
```rust
enum TypeError {
    Mismatch { expected: Type, found: Type, span: SourceSpan },
    MissingField { record: Type, field: Ident, span: SourceSpan },
    ExtraField { record: Type, field: Ident, span: SourceSpan },
    EffectEscape { decl_effect: Effect, actual_effect: Effect, span: SourceSpan },
    CapabilityRequired { cap: Ident, span: SourceSpan },
    UndefinedType { name: Path, span: SourceSpan },
    CyclicModule { path: Vec<Ident>, span: SourceSpan },
}
```

### Module Resolver Design

**Resolution algorithm:**
1. Collect all `@mod` declarations from all source files
2. Build identity table: map dotted paths → AST nodes
3. Walk each module's `@use` declarations:
   a. Resolve the target module identity
   b. Verify requested capabilities match `@pub` declarations
   c. If `@pub [read]` but module exports a function requiring `write` → capability error
4. Check for cycles (DFS with visited set)
5. Type-check each module in topological order (dependencies first)
6. Link into a combined module tree for evaluation

**File resolution:**
- `@use [read] from io` → looks up `aven/std/io` in the standard library, then local `io/` directory
- Standard library is embedded in the binary as strings or loaded from `AVEN_PATH`
- Resolver checks: embedded stdlib first, then filesystem relative to source file

### @diff Engine Design

**Three phases:**

1. **Parse** — the `@diff` block is parsed by a dedicated diff parser (reuses the AVEN lexer but has its own grammar rules for `@replace`, `@insert`, etc.)

2. **Resolve** — each selector path is resolved against the AST:
   - `/fn greet/arg name` → walk the root module → find `greet` → find param `name`
   - Handle ambiguity: if two params named `name`, use positional index: `name[0]`
   - Return `NodeId` of the target

3. **Apply** — for each patch operation:
   - `@replace` — detach old subtree at NodeId, attach new AST subtree
   - `@insert` — add new child at specified position (first/last/before/after)
   - `@delete` — remove subtree, re-link parent
   - `@move` — detach from source, attach at destination (check no cycles)
   - `@copy` — deep-clone subtree, attach at destination

**Validation after each patch:**
- The AST must remain well-formed (no dangling children, no duplicate NodeIds)
- The type checker re-checks the modified module
- If validation fails, the entire `@diffs` batch rolls back

**Rollback mechanism:** Before applying any patch operation, `clone()` the `Arc` handles pointing to affected subtrees and store them in a rollback list. Mutations use `Arc::make_mut` on the *working copy*, leaving the cloned handles untouched. If validation fails, swap the working copy back to the cloned handles. `Arc::make_mut` alone does not snapshot — the clone must be taken *before* calling it.

### Evaluator (Seed Runtime) Design

**AST-walking interpreter.**
- `eval(expr, &mut Env) -> Value`
- `Env` is a scoped hashmap: `[(String, Value)]` nested by lexical scope
- Tail recursion optimization for `@ret` at end of function

**Value representation:**
```rust
enum Value {
    Str(String),
    Int(i128),
    Flt(f64),
    Bool(bool),
    Symbol(String),
    Nil,
    Record(Vec<(String, Value)>),
    List(Vec<Value>),
    NativeFn(fn(&[Value], &mut Env) -> Result<Value, RuntimeError>),
    UserFn(NodeId, Arc<Env>),  // closure: fn AST node + captured environment; Arc (not Rc) so async eval can move across tasks
}
```

**Standard library intrinsics:** Registered as `NativeFn` values in the initial environment:
- `@io.write stdout "text"` → calls `print!`
- `@io.read stdin` → calls `read_line`
- `@fs.read path` → calls `std::fs::read_to_string`
- etc.

**Error handling:**
```rust
enum RuntimeError {
    TypeError { msg: String, span: SourceSpan },
    UndefinedVariable { name: String, span: SourceSpan },
    IOError { msg: String, span: SourceSpan },
    CapabilityViolation { msg: String, span: SourceSpan },
}
```

`@err expr` at runtime produces an error *value* — it does not unwind the call stack. The evaluator returns `Ok(Value::Err(box_value))` and the caller's `@match` or `?`-equivalent handles it. A function declared `~>` has return type `#ok T | #err E`; `@err` constructs the `#err` variant. Actual Rust-level `RuntimeError` is reserved for evaluator bugs (undefined variable, capability violation) — not for user-level `@err`.

### CLI Design

```text
aven <command> [options] [file]

Commands:
  run   <file>        Parse, type-check, and execute
  check <file>        Parse and type-check only (no execution)
  patch <file>        Apply @diff patches from stdin or --patch-file
  fmt   <file>        Canonical formatter (re-print AST to spec)
  repl                Interactive REPL (milestone)

Options:
  --patch-file <file>  Read @diff patches from file
  --emit-ast           Print AST before execution (debug)
  --emit-tokens        Print token stream before parsing (debug)
  --no-type-check      Skip type checking (fast path)
  --aven-path <dir>    Set standard library search path
```

---

## Incremental Build Plan (7 Milestones)

### Milestone 1: "Hello, AVEN" — Lexer + Parser + Basic Eval
**Files:** `span.rs`, `token.rs`, `ast.rs`, `lexer.rs`, `parser.rs`, `eval.rs`, `main.rs`

- Tokenize: `@fn`, `@ret`, `@call`, `@let`, identifiers, `Str`, `Int`, `::`, `->`, `+`, `(`, `)`, `{`, `}`, indentation
- Parse: function declarations, let bindings, calls, binary ops (string concat +, int `+`), literals (strings, ints), blocks
- Evaluate: function calls (no closures, top-level only), binary ops, let bindings
- `main.rs`: read file → lex → parse → eval → print result
- **Test:** `@fn greet :: name:Str -> Str @ret "Hello, " + name` prints `Hello, Alice`

### Milestone 2: Types & Type Checker
**Files:** `types.rs`, `check.rs`

- Type representations: primitives, records, unions, functions, generics, options, lists, caps
- Type checker: bidirectional checking, structural record matching, variant tag checking
- Effect tracking: verify body against declared effect
- Capability checking: module-level capability gating
- **Test:** type errors produce correct span + message

### Milestone 3: Control Flow + Pattern Matching
**Files:** extends `parser.rs`, `eval.rs`, `check.rs`

- `@if` / `@then` / `@else` parsing + eval
- `@match` parsing + eval (symbol match, value destructure, wildcard)
- `@err` expression creation + unwind
- All expression types get type-checked
- **Test:** full factorial using guard-based dispatch (integer literal patterns are not in the spec — `@match` only matches `#symbol` tags and wildcard `_`):
  ```aven
  @fn fact :: n:Int -> Int
    @if n == 0
      @then 1
      @else (n * (@call fact (n - 1)))
  ```
  Note: if integer literal patterns are desired, they must be added to the spec's pattern grammar before being implemented here.

### Milestone 4: Module System
**Files:** `module.rs`, extends `parser.rs`, `driver.rs`

- Multi-file compilation: collect `@mod` declarations from multiple `.aven` files
- Module graph: identity assignment, capability verification, cycle detection
- `@use [x y] from path` resolution
- `@pub` visibility
- Topological type-checking order
- **Test:** file A imports `[greet]` from file B, calls greet

### Milestone 5: @diff Engine
**Files:** `diff/selector.rs`, `diff/patch.rs`, `diff/mod.rs`, extends `main.rs`

- Selector path parser: `/fn greet/body/ret`, `/mod app/fn login/arg req`
- Patch application: parse `@diff` blocks, resolve selectors, apply operations with rollback
- `@diffs` batch with atomic semantics
- `.avenpatch` file support
- **Test:** parse module, apply `@replace`, verify AST changed correctly, type-check passes

### Milestone 6: Standard Library Intrinsics
**Files:** extends `eval.rs`

- Register native Rust functions for I/O, filesystem, basic math, collections
- `aven/std/io`: `print`, `read`, `write`
- `aven/std/fs`: `read_file`, `write_file`, `list_dir`
- `aven/std/collections`: `list.map`, `list.filter`, etc.
- `aven/std/math`: `+`, `-`, `*`, `/` as named functions for explicit calls
- **Test:** `@use [print] from io` → `@io.print "hello world"` works

### Milestone 7: Polish + Self-hosting Prep
- Error messages with span highlighting
- `aven fmt` — canonical formatter (re-print AST deterministically)
- `aven repl` — interactive REPL
- Full spec coverage audit (everything in `AVEN_SPEC.md` works)
- Write a non-trivial AVEN program (e.g., a JSON parser) to prove the language is useful
- Document all public APIs with `///` for rustdoc

---

## Key Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Parser | Hand-written recursive descent | Simpler than nom/pest for indentation-sensitive grammar; zero extra dependencies |
| AST lifetime | Owned (no arena) | Simpler to implement; patches need to swap subtrees freely |
| Inference | Bidirectional, not HM | HM is overkill for a seed; most types are explicit at boundaries |
| Runtime | AST interpreter, not bytecode | Simplest possible seed; fast enough for bootstrap |
| Selector addressing | Name path + positional fallback | `/fn greet/body` is readable; `/42` for disambiguation |
| Rollback for @diff | `Arc::clone` before mutation, restore on failure | Must clone the handle *before* `Arc::make_mut`, not after |
| Error values | `@err` → runtime unwind | No exception machinery; `@err` is just an early return with an error value |
| Standard library | Embedded Rust `NativeFn` | No FFI needed; stdlib functions are just Rust closures |
| Filesystem | Not required until Milestone 4 | Single-file AVEN works without module resolution |

---

## Dependencies (Cargo.toml)

```toml
[package]
name = "aven"
version = "0.1.0"
edition = "2024"

[dependencies]
# Minimal — avoid bloat for a seed compiler.
# clap for CLI arg parsing (optional, hand-rolled is fine)
# serde + serde_json for .avenpatch serialization
# That's it. Everything else is hand-rolled.
```

The seed compiler should have as few dependencies as possible — when the real compiler is written in AVEN, the Rust seed doesn't need to be maintained.
