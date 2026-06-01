# AST Encoding and Canonical Dump Format

This document specifies the canonical AVEN value representation for tokens and AST nodes, enabling span-stable serialization suitable for golden testing.

## Design Principles

1. **Span Elision**: `SourceSpan { start, end }` and `NodeId` fields are omitted from canonical dumps, ensuring output stability across re-parses with fresh IDs.
2. **Tagged Record Format**: Each Token and Expr variant is represented as a record with `@kind` field denoting the variant, plus variant-specific data fields.
3. **Recursive Encoding**: Complex AST nodes recursively encode their subexpressions in the same format.
4. **Newline Delimiters**: Collections of AST nodes (e.g., function parameters, block statements) are `\n`-joined in Str form, matching seed stdlib conventions.

## Token Encoding

Each `Token` variant maps to a single-line canonical representation:

### Sigils and Keywords (no data)
```
At
Hash
Underscore
Let
Fn
Type
Ret
If
Then
Else
True
False
IoWrite
Call
Cap
Use
From
Mod
Pub
Match
Ok
Err
As
Intent
Uncertain
Ctx
CtxGet
CtxSet
Diff
Diffs
Replace
Insert
Delete
Move
Copy
Meta
To
First
Last
Before
After
PatchFor
Colon
DoubleColon
Equals
Arrow
Comma
LeftParen
RightParen
LeftBrace
RightBrace
LeftBracket
RightBracket
Pipe
Plus
Minus
Star
Slash
Question
Eof
Newline
```

### Tokens with Data
```
EffectArrow <effect-sigils>
Ident <name>
Integer <value>
Float <value>
String <quoted-value>
```

Where `<effect-sigils>` is either `[]` for no effects, or a sequence of sigils: `?` for error, `!` for IO, `~` for async (e.g., `?!` for error+IO).

**Example canonical token dump**:
```
LeftParen
Plus
Integer 42
String "hello"
RightParen
Eof
```

## AST Node Encoding

Each `Expr` variant is wrapped in parentheses as a parenthesized S-expression. NodeId and SourceSpan are elided. All structural delimiters are `(` and `)` only—never `{` or `}`.

### Int, Float, Str, Bool (literal expressions)
```
(Int <value>)
(Float <value>)
(Str "<quoted-value>")
(Bool <true|false>)
```

### Nil (standalone)
```
(Nil)
```

### Symbol, Var (tagged value)
```
(Symbol <tag-name>)
(Var <var-name>)
```

### Let (binding)
```
(Let <name> <expr>)
```

**Example**: `@let x :: 10` encodes as:
```
(Let x (Int 10))
```

### FnDef (function definition)
```
(FnDef <name> (<params>) <body> <return-type> <effects> [<caps>])
```

- `<params>`: Space-separated parameter specs, each `(<param-name>)` or `(<param-name> <type>)`
- `<body>`: Nested expression via dump_expr
- `<return-type>`: Canonical type encoding or `(Nil)` if absent
- `<effects>`: Sigil string (`[]` or e.g. `?!~`) representing error/io/async flags
- `<caps>`: Optional space-separated capability names (omitted if empty)

**Example**: `@fn add :: a:Int b:Int -> Int @ret (+ a b)` encodes as:
```
(FnDef add ((a Int) (b Int)) (Ret (Arithmetic Add (Var a) (Var b))) Int [])
```

### FnCall (function invocation)
```
(FnCall <func-name> <arg1> <arg2> ...)
```

**Example**: `(add 1 2)` encodes as:
```
(FnCall add (Int 1) (Int 2))
```

> Note: the arithmetic operators `+ - * /` do NOT parse to `FnCall`. `(+ 2 3)` parses to `Expr::Arithmetic{Add}` and encodes as `(Arithmetic Add (Int 2) (Int 3))` (see the Arithmetic section). Only non-operator heads produce `FnCall`.

### If (conditional)
```
(If <cond> <then-expr> <else-expr>)
```

**Example**: `@if @true @then 100 @else 0` encodes as:
```
(If (Bool true) (Int 100) (Int 0))
```

### Arithmetic (binary operation)
```
(Arithmetic <op> <left> <right>)
```

**Example**: `(+ 1 2)` encodes as:
```
(Arithmetic Add (Int 1) (Int 2))
```

### Ret, IoWrite, Uncertain (single-argument wrappers)
```
Ret <expr>
IoWrite <expr>
Uncertain <expr>
```

### Block (sequence of expressions)
```
Block <exprs-str>
```

- `exprs-str`: `\n`-joined sequence of subexpressions

### Intent (annotation)
```
Intent <intent-name>
```

### Ctx (context access)
```
Ctx
```

### CtxGet, CtxSet (context operations)
```
CtxGet ctx::<expr> key::<expr>
CtxSet ctx::<expr> key::<expr> value::<expr>
```

### Match (pattern matching)
```
Match scrutinee::<expr> patterns::<patterns-str>
```

- `patterns-str`: `\n`-joined list of `<pattern> -> <expr>`
- Pattern: `Tag <tag-name>`, `TagBind <tag-name> <var-name>`, or `Wildcard`

**Example**: `@match x @case (#ok v) -> v @case _ -> 0` encodes as:
```
Match scrutinee::Var x patterns::TagBind ok v -> Var v\nWildcard -> Int 0
```

### Tagged (tagged value construction)
```
Tagged tag::<tag-name> payload::<expr|null>
```

### Mod, Use, Pub, PubDecl, TypeAlias, Record, List, Diff
Encoded with variant name and field list, following the same tagged-record pattern.

**Example**: `@type MyType = Int` encodes as:
```
TypeAlias name::MyType type_params:: ty::Int
```

## Span Elision Rules

For all variants carrying `node_id: NodeId` or `span: SourceSpan` fields:
- Do NOT print the field name or value.
- Only serialize data-bearing fields (names, values, subexpressions, types).

## Canonical Dump Implementation

The `canonical_token_dump` and `canonical_ast_dump` functions produce newline-delimited output:
- One token per line for token dumps.
- One node per line (depth-first) for AST dumps, with indentation to denote nesting depth (if desired) or flattened with structure markers.

**Design choice**: Favor flat, one-record-per-line format for simplicity and diff-ability.

## Stability Guarantees

- Canonical dumps are deterministic: same source always produces same dump.
- Dumps omit spans and node IDs, so re-parsing the same source produces identical dumps.
- Dumps can serve as golden files for regression testing.
