# Seed-AVEN Dialect Specification

This document defines the exact dialect accepted by the seed AVEN compiler, as implemented in the Rust seed source.

## Core Syntax Constraints

### 1. Arithmetic Expressions (Prefix-Only)

- **Constraint**: All binary arithmetic uses prefix notation; infix operators are forbidden.
- **Source verification**: `src/parser.rs` implements the `parse_expression` (line 1335) and `parse_statement` functions with pattern matching on tokens. Infix operators (`+`, `-`, `*`, `/`) are parsed only as function call arguments, never as infix operators in expressions.
- **Examples**:
  - Valid: `(+ 2 3)`, `(* (+ 1 2) 4)`
  - Invalid: `2 + 3`, `1 * 2 + 3` (not accepted)

### 2. Pattern Matching Restrictions

- **Constraint**: `@match` scrutinee dispatches on symbol tags (`#tag`) only; patterns are `Tag(String)` for bare tag match, `TagBind(String, String)` for tag-with-payload bind, or `Wildcard` for `_`.
- **Source verification**: `src/ast.rs` lines 307–312 define `enum Pattern { Tag(String), TagBind(String, String), Wildcard }`. `src/parser.rs` parse_match only creates these variants.
- **Examples**:
  - Valid: `@match x @case (#ok v) -> v @case _ -> 0`
  - Invalid: `@match x @case (42) -> "nope"` (literals as patterns forbidden)

### 3. Collection Literals

- **Constraint**: No native `[T]` or `{}` literal syntax. Collections are `\n`-joined `Str` values (AVEN level) or `Value::Map`/`Value::Set` (runtime level).
- **Source verification**: `src/lexer.rs` lines 73–75 define `LeftBracket`, `RightBracket` tokens for reserved syntax; `src/parser.rs` does not implement array/set/map literals. `src/eval.rs` and stdlib provide `\n`-joined `Str` conventions.
- **Examples**:
  - Valid: `"item1\nitem2\nitem3"` (Str with newline separators)
  - Invalid: `[1 2 3]`, `{key: value}` (forbidden at parse time)

### 4. Block Grammar

- **Constraint**: Block expressions use indent-delimited bodies; top-level statements are separated by `\n`.
- **Source verification**: `src/parser.rs` implements `parse_program` and `parse_statement` which process sequences of `Expr` delimited by newlines. Indentation is tracked but not enforced as a syntax error; the parser collects expressions into a `Block(Vec<Expr>, ...)` variant.
- **Examples**:
  - Valid:
    ```
    @fn foo -> Int
    (+ 1 2)
    (+ 3 4)
    ```
  - Semantics: Multiple expressions in sequence; returns last.

## AST Variants Summary

| Expr Variant | Fields | Notes |
|---|---|---|
| `Int` | `(i64, NodeId, SourceSpan)` | Integer literal |
| `Float` | `(f64, NodeId, SourceSpan)` | Float literal |
| `Str` | `(String, NodeId, SourceSpan)` | String literal |
| `Bool` | `(bool, NodeId, SourceSpan)` | Boolean (`@true` / `@false`) |
| `Nil` | none | `@nil` keyword |
| `Symbol` | `(String, NodeId, SourceSpan)` | Tag symbol `#tag` |
| `Var` | `(String, NodeId, SourceSpan)` | Variable identifier |
| `Let` | `{ name, value, node_id, span }` | Binding: `@let x :: expr` |
| `FnDef` | `{ name, params, body, return_type, effect_level, cap, node_id, span }` | Function definition |
| `FnCall` | `{ name, args, node_id, span }` | Function call |
| `If` | `{ cond, then_branch, else_branch, node_id, span }` | Conditional |
| `Arithmetic` | `{ op, left, right, node_id, span }` | Binary arithmetic (prefix) |
| `Ret` | `(Box<Expr>, NodeId, SourceSpan)` | Return statement |
| `IoWrite` | `(Box<Expr>, NodeId, SourceSpan)` | I/O write operation |
| `Block` | `(Vec<Expr>, NodeId, SourceSpan)` | Sequence of expressions |
| `Intent` | `(String, NodeId, SourceSpan)` | Intent annotation |
| `Uncertain` | `(Box<Expr>, NodeId, SourceSpan)` | Uncertainty wrapper |
| `Ctx` | `{ node_id, span }` | Context access |
| `CtxGet` | `{ ctx, key, node_id, span }` | Context key lookup |
| `CtxSet` | `{ ctx, key, value, node_id, span }` | Context key assignment |
| `Diff` | `{ metadata, ops, node_id, span }` | Diff operator collection |
| `Use` | `{ caps, module, node_id, span }` | Module/capability use |
| `Match` | `{ scrutinee, patterns, node_id, span }` | Pattern match |
| `Tagged` | `{ tag, payload, node_id, span }` | Tagged value |
| `Mod` | `{ name, node_id, span }` | Module declaration |
| `Pub` | `{ cap, node_id, span }` | Pub keyword marker |
| `PubDecl` | `{ inner, node_id, span }` | Pub wrapper for declarations |
| `TypeAlias` | `{ name, type_params, ty, node_id, span }` | Type alias |
| `Record` | `{ fields, node_id, span }` | Record literal |
| `List` | `{ elements, node_id, span }` | List literal |

## Token Variants Summary (66 total)

| Token Variant | Meaning |
|---|---|
| `At`, `Hash`, `Underscore` | Sigils |
| `Let`, `Fn`, `Type`, `Ret`, `If`, `Then`, `Else` | Control keywords |
| `True`, `False` | Boolean literals |
| `IoWrite`, `Call`, `Cap`, `Use`, `From`, `Mod`, `Pub`, `Match`, `Ok`, `Err`, `As` | Other keywords |
| `Intent`, `Uncertain`, `Ctx`, `CtxGet`, `CtxSet` | Annotation keywords |
| `Diff`, `Diffs`, `Replace`, `Insert`, `Delete`, `Move`, `Copy`, `Meta`, `To`, `First`, `Last`, `Before`, `After`, `PatchFor` | Diff sub-grammar |
| `Colon`, `DoubleColon`, `Equals`, `Arrow` | Separators |
| `EffectArrow(EffectSet)` | Effect markers |
| `Comma`, `LeftParen`, `RightParen`, `LeftBrace`, `RightBrace`, `LeftBracket`, `RightBracket`, `Pipe` | Delimiters |
| `Plus`, `Minus`, `Star`, `Slash`, `Question` | Operators |
| `Ident(String)`, `Integer(i64)`, `Float(f64)`, `String(String)` | Literals with data |
| `Eof`, `Newline` | Special tokens |

## Comment Syntax

Comments begin with `;` and extend to end-of-line.

**Source verification**: `src/lexer.rs` implements `skip_comment()`, which skips input when `current()` is `;` until newline.

## Comparison & Boolean Primitives

The seed evaluator provides comparison and boolean logic builtins to enable loop bounds, range tests, and conditional branching (added in stage S2.0b).

### Integer Comparison

All integer comparisons take two `Int` arguments and return `Bool`:
- `int_eq` — equality: `(@call int_eq 5 5)` → `@true`
- `int_lt` — less-than: `(@call int_lt 3 5)` → `@true`
- `int_gt` — greater-than: `(@call int_gt 5 3)` → `@true`
- `int_le` — less-or-equal: `(@call int_le 5 5)` → `@true`
- `int_ge` — greater-or-equal: `(@call int_ge 5 5)` → `@true`

**Type error**: passing non-`Int` arguments returns `EvalError::TypeError`.

**Source verification**: `src/eval.rs` lines 1951–2049, each builtin registered with `env.define()` and matching on `(Value::Int(a), Value::Int(b))`.

### Boolean Logic

- `bool_and` (arity 2) — logical AND: `(@call bool_and @true @false)` → `@false`
- `bool_or` (arity 2) — logical OR: `(@call bool_or @true @false)` → `@true`
- `bool_not` (arity 1) — logical NOT: `(@call bool_not @true)` → `@false`

**Type error**: passing non-`Bool` arguments returns `EvalError::TypeError`.

**Source verification**: `src/eval.rs` lines 2051–2110, matching on `Value::Bool`.

### String/Character Ordering

All string comparisons use lexicographic Unicode ordering (applies to single-char strings from `str_get`):
- `str_lt` — less-than: `(@call str_lt "a" "b")` → `@true`
- `str_gt` — greater-than: `(@call str_gt "z" "a")` → `@true`
- `str_le` — less-or-equal: `(@call str_le "a" "a")` → `@true`
- `str_ge` — greater-or-equal: `(@call str_ge "z" "z")` → `@true`

**Type error**: passing non-`Str` arguments returns `EvalError::TypeError`.

**Source verification**: `src/eval.rs` lines 2113–2191, matching on `(Value::Str(a), Value::Str(b))`.

### S2.0b Builtin Summary

The following builtins were added in stage S2.0b and are available in the seed evaluator:

| Category | Functions |
|---|---|
| Integer Comparison | `int_eq`, `int_lt`, `int_gt`, `int_le`, `int_ge` |
| Boolean Logic | `bool_and`, `bool_or`, `bool_not` |
| String Ordering | `str_lt`, `str_gt`, `str_le`, `str_ge` |
| String Primitives (earlier stage) | `str_eq`, `str_get`, `str_len`, `str_sub` |

**Note**: String comparison operators use lexicographic Unicode ordering consistent with `str_eq`.

## Dialect Table Summary

| Feature | Supported | Reference |
|---|---|---|
| Prefix arithmetic | Yes | `src/parser.rs` parse_expression |
| Infix operators | No | `src/parser.rs` forbids infix parsing |
| Symbol-only pattern matching | Yes | `src/ast.rs` Pattern enum |
| Literal-based patterns | No | `src/parser.rs` parse_match only creates Pattern::{Tag,TagBind,Wildcard} |
| Array/Set/Map literals | No | `src/lexer.rs` tokens defined but `src/parser.rs` forbids |
| `\n`-joined Str collections | Yes | `src/eval.rs` stdlib conventions |
| Indent-delimited blocks | Yes | `src/parser.rs` parse_program/parse_statement |
| `;` comments | Yes | `src/lexer.rs` skip_comment() |
