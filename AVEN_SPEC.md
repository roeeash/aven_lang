# AVEN Specification

> Agent Vector Expression Notation — v0.1

---

## 1. Type System

### 1.1 Principles

| Principle | Meaning |
|---|---|
| **Effect-typed** | Types encode what a function can *do* (err, IO, async) via arrow level — `->` vs `~~>` is part of the type |
| **Canonical** | One way to annotate any value — `::` is the universal separator |
| **Explicit effects** | No hidden IO, no hidden state — effects are part of the type |
| **Nominal + Structural** | Named types for APIs; structural compatibility for composition |
| **Composition over inheritance** | No subtyping, no class hierarchies |

### 1.2 Primitives

| Type | Notation | Examples |
|---|---|---|
| String | `Str` | `"hello"`, `"42"` |
| Integer | `Int` | `0`, `-42`, `1_000_000` |
| Float | `Flt` | `3.14`, `-0.001` |
| Boolean | `Bool` | `@true`, `@false` |
| Symbol | `#<name>` | `#admin`, `#get`, `#pending` |
| Nothing | `Nil` | (unit type, single value `_`) |

Symbols (`#`) are a first-class type — they are interned, comparable constants. Used for tags, enum variants, protocol identifiers.

### 1.3 Compound Types

**Record (product type):**
```aven
{name:Str age:Int role:#role}
# field order is canonical — sorted alphabetically
```

**Union (sum type):**
```aven
#ok value:Str | #err msg:Str
# tagged union — each variant prefixed with a symbol
```

**List:**
```aven
[Int]
[Str]
[{name:Str}]
```

**Option:**
```aven
?Str        ; Str | Nil
?Int        ; Int | Nil
```

### 1.4 Function Types

```aven
; Unary
Str -> Int

; Multi-argument (curried by default)
Str -> Int -> Bool

; Named arguments (record input)
{name:Str age:Int} -> Str

; Typed effect signature — effect flags on the arrow, canonical order: ?!~
Str ->    Int    ; pure
Str -?>   Int    ; may err
Str -!>   Int    ; IO only
Str -~>   Int    ; async only
Str -?!>  Int    ; may err + IO
Str -?~>  Int    ; may err + async
Str -!~>  Int    ; IO + async
Str -?!~> Int    ; may err + IO + async
```

Effect flags are **orthogonal** — each is independently present or absent. The three flags and their meanings:

| Flag | Effect | Meaning |
|---|---|---|
| `?` | err | function may return an error value (`#ok T \| #err E`) |
| `!` | IO | function reads or writes external state (filesystem, network, stdout) |
| `~` | async | function returns a future; caller must await |

Flags always appear in canonical order `?!~`. Omitting a flag means the effect is absent.

All 8 combinations:

| Arrow | Err | IO | Async |
|---|---|---|---|
| `->` | | | |
| `-?>` | ✓ | | |
| `-!>` | | ✓ | |
| `-~>` | | | ✓ |
| `-?!>` | ✓ | ✓ | |
| `-?~>` | ✓ | | ✓ |
| `-!~>` | | ✓ | ✓ |
| `-?!~>` | ✓ | ✓ | ✓ |

Effect enforcement: a caller with effect set S may only call a callee whose effect set is a **subset** of S. A pure (`->`) function cannot call anything with any flag. An IO-only (`-!>`) function cannot call an async (`-~>`) function. No implicit effects — a function declared `->` cannot perform IO, err, or async.

The `@cap` constraint ties capability markers to the effect set: a function annotated `@cap write` must carry the `!` flag (`-!>` or higher). The checker rejects a `->` function claiming a write capability.

### 1.5 Type Aliases

```aven
@type UserId = Int
@type Json    = Str
@type Handler = {req:Request} ~~> {res:Response}
```

### 1.6 Type Parameters (Generics)

```aven
@type Pair a b = {first:a second:b}
@fn id t :: a -> a
  @ret t

@type Result ok err = #ok ok | #err err
```

Type parameters are lowercase single-identifier by convention. No bounds — concrete at instantiation. Variance is determined by position in function types (see §1.9).

### 1.7 Type Inference

Type annotations are optional at call sites but required at public boundaries (`@fn`, `@let` at module level). Local bindings inside function bodies may omit types:

```aven
@fn double :: x:Int -> Int
  @let y = x * 2    ; inferred: Int
  @ret y
```

### 1.8 Capability Types

Types can carry capability markers that constrain what code can do:

```aven
@type Readable  = @cap read
@type Writable  = @cap write
@type Loggable  = @cap log

@fn process :: input:Readable -> @cap write Str
```

`@cap` is a constraint on the function's effect level: a function annotated `@cap write` in its return type must have an effect arrow of `~~>` or higher (IO-capable). The checker rejects a `->` function that claims a write capability. This ties capability markers directly to the effect arrow system in §1.4.

### 1.9 Type Equality & Compatibility

- Records are structurally compatible (field-by-field matching)
- Unions match by variant tag
- Function types match contravariantly on input, covariantly on output
- `@type` creates a nominal alias — `UserId` and `Int` are **not** interchangeable unless explicitly unwrapped

---

## 2. `@diff` Patch Format

### 2.1 Purpose

`@diff` is a **semantic patch format** that describes changes to AVEN ASTs at the node level. Unlike text diffs (unified diff, which operates on lines), `@diff` operates on typed tree nodes, making patches:
- **Position-independent** — a patch targets a node by its path, not its line number
- **Merge-safe** — non-overlapping patches to sibling subtrees never conflict
- **Structured output** — `@diff` blocks are machine-emittable; the format is designed to be generated without free-form text editing

### 2.2 Patch Target Addressing

Every node in an AVEN AST has a canonical **selector** path:

```aven
/                        ; root module
/fn greet                ; function named 'greet'
/fn greet/arg name       ; argument 'name' of function 'greet'
/fn greet/body/ret       ; return expression inside greet
/type User               ; type alias User
/mod db                  ; submodule 'db'
```

### 2.3 Patch Operations

| Operation | Syntax | Semantics |
|---|---|---|
| Replace subtree | `@replace <selector> <new-ast>` | Swap entire subtree |
| Insert child | `@insert <selector> <position> <ast>` | Add new sibling/child |
| Delete subtree | `@delete <selector>` | Remove node and its subtree |
| Move subtree | `@move <src> -> <dst>` | Relocate subtree |
| Copy subtree | `@copy <src> -> <dst>` | Duplicate subtree |

### 2.4 Full Format

```aven
@diff
  @meta
    description: "add admin auth check to greet endpoint"
    author:      agent-session-7f3a
    timestamp:   2026-05-12T14:22:00Z

  @replace /fn greet/body
    @let user = @call auth.check req.token
    @match user.role
      #admin -> @call greet.handle req
      _      -> @err "forbidden"

  @insert /fn greet/arg req position:first
    req:Request

  @delete /fn greet/arg name
```

### 2.5 Position for Insert

| Position value | Meaning |
|---|---|
| `first` | Insert before all existing children |
| `last` | Insert after all existing children |
| `before:<selector>` | Insert immediately before target sibling |
| `after:<selector>` | Insert immediately after target sibling |

### 2.6 Batch Diffs

Multiple `@diff` blocks can be composed into a patch set:

```aven
@diffs
  @diff ...  ; patch 1
  @diff ...  ; patch 2
```

Application order is sequential. If any patch fails, the entire `@diffs` batch is rejected (atomic).

### 2.7 Validation Rules

- A `@replace` target must exist (error if not found)
- A `@delete` target must exist
- An `@insert` target parent must exist
- A `@move` source must exist and destination must not exist
- After application, the result must be a valid AVEN module (type-checked)

### 2.8 Relation to Git/File System

`@diff` patches are **not** tied to files — they operate on the semantic module graph. A single `@diff` can span multiple files if the module system merges them into one AST (see §3.6).

For file-level storage, patches can be serialized to `.avenpatch` files:

```aven
; auth_check.avenpatch
@patch-for path:"src/greet.av"
  @diff ...
```

---

## 3. Module System

### 3.1 Principles

| Principle | Meaning |
|---|---|
| **Capability-based** | Import what you need, and only what you need |
| **Resolver-independent** | Modules are addressed by semantic identity — filesystem layout is an implementation detail of the resolver |
| **Explicit context** | No global state — all shared context is threaded via `@ctx` |
| **Tree-structured** | Modules form a tree, not a graph (no circular deps) |
| **Effect-isolated** | Module boundaries track effect propagation |

### 3.2 Module Declaration

```aven
; root module — no name, file is the identity
@mod
  @use [read write] from fs
  @fn greet ...

; named submodule
@mod parser
  @use [read] from fs
  @fn parse ...
```

### 3.3 Import Syntax

```aven
; Import specific capabilities from a module
@use [read write] from fs

; Import everything (discouraged, but valid for bootstrapping)
@use * from http

; Import with rename
@use [get as fetch] from http

; Import submodule
@use [read] from fs.file
```

### 3.4 Module Identity

Modules are identified by a canonical dotted path:

```aven
aven/std/io       ; standard library IO
aven/std/fs       ; standard library filesystem
app/services/auth ; application auth service
```

The identity is **independent** of filesystem layout. A resolver maps identities to locations (local files, registry, inline definitions).

### 3.5 Exports

By default, everything is private. Explicit export:

```aven
@mod
  @pub @fn greet ...         ; public function
  @pub @type Handler ...     ; public type

  @fn helper ...              ; private — not importable
```

Granular capability export:

```aven
@mod db
  @pub [read write]          ; only these capabilities are accessible
  @pub @type Connection ...

  @fn connect ...            ; also public, callers get read+write capability
```

### 3.6 Module Graph & Compilation

Modules form a **directed acyclic graph** (no cycles). The compiler:

1. Resolves all `@use` declarations to module identities
2. Builds the full module tree
3. Type-checks each module against its imported capabilities
4. Links into a single combined AST or separate compilation units

A `@diff` patch can target any module in the tree:

```aven
@diff
  @replace /mod app/services/auth/fn login/body
    ...
```

The selector path respects module nesting.

### 3.7 Context Threading (`@ctx`)

Modules cannot access global state. Shared context (config, database handles, request scopes) is explicitly threaded:

```aven
@fn handle_request :: ctx:Context req:Request ~~> Response
  @let db   = @ctx.get ctx "db"
  @let user = @call db.query ...
```

Context is typed — `@ctx.get` returns a `?T` and must be matched or unwrapped.

### 3.8 Standard Library

The standard library (`aven/std/`) provides:

| Module | Capabilities | Description |
|---|---|---|
| `io` | `[read write print]` | Console I/O |
| `fs` | `[read write list]` | Filesystem operations |
| `http` | `[get post put delete]` | HTTP client |
| `json` | `[parse serialize]` | JSON handling |
| `time` | `[now sleep format]` | Time utilities |
| `math` | `[calc]` | Math operations |
| `collections` | `[list map set]` | Data structure operations |

### 3.9 Module Resolution Algorithm

```
1. Collect all @mod declarations in the source
2. Assign each module its identity (dotted path)
3. Collect all @use declarations
4. For each @use:
   a. Resolve the module identity via lookup table
   b. Verify requested capabilities are in @pub of target
   c. Bind capabilities to local scope
5. Check for cycles — fail if any
6. Type-check each module with its capabilities resolved
7. Compile
```

---

## Appendix A: Type Grammar (Informal)

```ebnf
type-expr    = primitive-type
             | record-type
             | union-type
             | list-type
             | option-type
             | fn-type
             | type-ref
             | type-param

primitive-type = "Str" | "Int" | "Flt" | "Bool" | "Nil"
record-type    = "{" field-expr (" " field-expr)* "}"
field-expr     = ident ":" type-expr
union-type     = variant ("|" variant)*
variant        = "#" ident value-type?
value-type     = ident ":" type-expr
list-type      = "[" type-expr "]"
option-type    = "?" type-expr
fn-type        = type-expr arrow type-expr
arrow          = "-" effect-flags ">"
effect-flags   = "?"? "!"? "~"?   ; flags in canonical order; all optional
type-ref       = ident ("." ident)*
type-param     = lowercase-ident
```

## Appendix B: Diff Grammar (Informal)

```ebnf
diff           = "@diff" meta? patch+
meta           = "@meta" field+
patch          = replace | insert | delete | move | copy
replace        = "@replace" selector expr
insert         = "@insert" selector position expr
delete         = "@delete" selector
move           = "@move" selector "->" selector
copy           = "@copy" selector "->" selector
selector       = "/" (ident ("/" ident)*)?
position       = "first" | "last"
               | "before:" selector
               | "after:" selector
```

## Appendix C: Module Grammar (Informal)

```ebnf
module         = "@mod" module-name? decl*
module-name    = ident
decl           = fn-decl | type-decl | use-decl | mod-decl | pub-decl
use-decl       = "@use" use-target "from" module-ref
use-target     = "*" | "[" ident (" as " ident)? ("," ident)* "]"
module-ref     = ident ("." ident)*
pub-decl       = "@pub" decl
mod-decl       = "@mod" ident decl*
fn-decl        = "@fn" ident "::" fn-sig body
type-decl      = "@type" ident type-params? "=" type-expr
```
