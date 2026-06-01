# AVEN Compiler (written in AVEN)

This directory contains the bootstrap AVEN compiler, written in AVEN itself, targeting the seed-AVEN subset defined in [SUBSET.md](SUBSET.md).

## Overview

The AVEN compiler is a self-hosted compiler that:
- Reads source code in the AVEN language (a Lisp-prefix syntax with pattern matching and capability-based effects)
- Performs lexical analysis (tokenization), parsing, type checking, and evaluation
- Outputs AVEN bytecode or executes directly

This is the target implementation for Milestones S2.1 through S2.8 of the AVEN bootstrapping roadmap.

## Modules

Each module corresponds to a compiler phase:

- **lexer.aven**: Tokenization — splits source text into `Token` stream
- **parser.aven**: Parsing — converts `Token` stream into abstract syntax tree (`Expr`)
- **check.aven**: Type checking — validates type safety and capability constraints
- **eval.aven**: Evaluation/interpretation — executes AST with environment/context tracking
- **diff.aven**: Diff/patch operations — handles code transformation and structured edits
- **module.aven**: Module system — manages imports, exports, and capability delegation
- **driver.aven**: Build driver — orchestrates compilation pipeline
- **main.aven**: Entry point — CLI and REPL

## Testing

Tests are located in `tests/`. The testing strategy uses golden-file regression tests:
- Canonical token and AST dumps are compared against expected output
- Error cases are validated for graceful failure modes

See [S2.1 milestone](../ROADMAP.md) for the golden-test harness implementation.

## Language Subset

The compiler targets the seed-AVEN dialect documented in [SUBSET.md](SUBSET.md):
- Prefix-only arithmetic: `(+ a b)` instead of `a + b`
- Symbol-tag pattern matching: `@match x @case #ok -> ...`
- Newline-separated collections: `"item1\nitem2\nitem3"`
- Indent-delimited blocks: Multi-line function bodies

## AST Representation

The abstract syntax tree and token stream are canonically encoded per [AST_ENCODING.md](AST_ENCODING.md) to ensure diff-stable serialization for golden testing.

## Build and Development

To build and test the seed (Rust) compiler:
```bash
cd ..
cargo build
cargo test
```

To test the AVEN compiler (once bootstrap is complete):
```bash
../target/debug/aven main.aven verify <file.aven>
../target/debug/aven main.aven emit-tokens <file.aven>
../target/debug/aven main.aven emit-ast <file.aven>
```
