# AVEN Seed Interpreter - Implementation Summary

## Supported Language Subset

### Core Literals
- **Integers**: `42`, `-7`, `1_000` (with underscores)
- **Strings**: `"hello"` (double-quoted, `\n`, `\t`, `\\`, `\"`)
- **Booleans**: `@true`, `@false`
- **Nil**: `_`
- **Symbols**: `#admin`, `#get`, etc.

### Declarations & Bindings
- `@let name :: expr` - Value binding
- `@fn name :: args -> RetType` - Function definition with optional type annotations

### Function Calls
- `@call name arg1 arg2 ...` - Function invocation

### Control Flow
- `@if cond @then expr @else expr` - Conditional (all branches required)

### Operators (Lisp-style, prefix notation)
- `(+ a b)` - Addition (Int or Str)
- `(- a b)` - Subtraction (Int only)
- `(* a b)` - Multiplication (Int only)
- `(/ a b)` - Division (Int only, truncates, error on divide-by-zero)

### I/O
- `@io.write expr` - Print expression result to stdout

### Other
- `@ret expr` - Return from function
- `;` comment to end of line

## Implementation Details

### File-by-File Summary

| File | Purpose |
|------|---------|
| `Cargo.toml` | Rust package manifest (edition 2021, no external dependencies) |
| `src/lib.rs` | Public API: `run_str()`, `Parser`, `Env`, `Value`, `eval()` |
| `src/main.rs` | REPL binary with interactive prompt |
| `src/ast.rs` | AST: `Expr` and `ArithOp` enums |
| `src/lexer.rs` | Hand-rolled tokenizer (no regex, 362 lines, 6 unit tests) |
| `src/parser.rs` | Recursive-descent parser (408 lines, 6 unit tests) |
| `src/eval.rs` | Tree-walking evaluator with environment (286 lines, 7 unit tests) |
| `tests/integration.rs` | End-to-end tests (107 lines, 15 tests) |
| `README.md` | User-facing documentation |

### Test Coverage

- **Lexer**: keywords, integers, strings, symbols, comments, operators (6 tests)
- **Parser**: primitives, let, arithmetic, if-expressions (6 tests)
- **Evaluator**: literals, let, variables, arithmetic, conditionals, function def/call (7 tests)
- **Integration**: complete programs, multiple statements, all operators (15 tests)
- **Total**: 34 tests

## Design Decisions

1. **No external dependencies**: Hand-rolled lexer and parser for minimal complexity and maximum portability.
2. **Tree-walking interpreter**: Simple, correct evaluator with environment-based scoping and closure support.
3. **Type annotations ignored**: Parsed but not enforced—seed is untyped at runtime.
4. **Lisp-style operators**: Eliminates precedence ambiguity; all binary ops require parentheses.
5. **Variable references**: Identifiers (not prefixed with `@`) are variable lookups.
6. **IoWrite accepts expressions**: `@io.write` can take any expression, not just string literals.

## Known Limitations

1. **No type checking**: Annotations parsed but not validated.
2. **No pattern matching**: `@match` not implemented.
3. **No composite types**: Records, unions, lists not supported.
4. **No modules**: `@mod`, `@use` not implemented.
5. **No async/effects**: No effect typing or async constructs.
6. **Single-line REPL**: Interactive shell takes one line per input.
7. **Limited error messages**: No source location tracking.
8. **No string interpolation**: Use `+` for concatenation.

## Build & Test

### Requirements
- Rust 1.56+ (2021 edition)
- `cargo` package manager

### Building
```bash
cd /Users/roee.ashkenazi/Desktop/AVEN/seed
cargo build --release
```

### Testing
```bash
cargo test
```

### Running REPL
```bash
cargo run --bin aven
```

## Example Programs

### Factorial (via recursion not yet tested, but closures work)
```aven
@fn add :: a:Int b:Int -> Int
  @ret (+ a b)
@call add 3 4
```

### Conditional
```aven
@if @true @then "yes" @else "no"
```

### Let + Function
```aven
@let x :: 5
@fn sq :: n:Int -> Int
  @ret (* n n)
@call sq x
```

## Code Quality

- **No panics**: All errors are `Result<Value, EvalError>` or `Result<Expr, ParseError>`.
- **Proper closures**: Function definitions capture their defining environment.
- **Recursive descent**: Clean, maintainable parser structure.
- **Modular design**: Lexer, parser, evaluator clearly separated.

## What Was Cut and Why

1. **Pattern matching (`@match`)**: Added significant complexity to AST and evaluator; deferred to Stage 2.
2. **Records & unions**: Required type system; seed is dynamically typed.
3. **Modules**: Scoping and resolution logic not essential for seed; deferred.
4. **Effect types**: Type checking deferred; seed runs untyped.
5. **Async/await**: Runtime complexity; not needed for seed.
6. **Symbols as first-class**: Parsed as `Expr::Symbol` but not fully integrated (sufficient for seed).

## Future Enhancements

- Type inference and checking
- Pattern matching and destructuring
- Record/union types and list operations
- Module system with imports
- Effect typing (->`, `~>`, `~~>`, `~~~>`)
- Optimization (constant folding, inline expansion)
- WASM compilation target
