# Why AVEN?

## 1. Canonical Vocabulary Eliminates Generation Choice

Every syntactic construct in AVEN has exactly one spelling. Within any given language this is already true — Python uses `def`, Rust uses `fn`. The problem is what happens *inside* a language: anonymous vs. named functions, `if/else` chains vs. `match`, implicit vs. explicit returns, braces vs. indentation, optional semicolons. These are all semantically equivalent forms that a code-generating agent must choose between — and choice is a failure mode.

AVEN eliminates the choice space entirely. Every binary expression requires explicit parens. Every return is `@ret`. Every function is `@fn`. There is no equivalent form to drift toward. AVEN produces the same token sequence for the same program, every time, from any agent.

The secondary win: **`@diff` eliminates sending entire files in edit loops.** An agent sends `@replace /fn greet/body` + 3 lines instead of re-transmitting a 200-line file. In an iterative coding loop (generate → feedback → edit → feedback), this compounds across sessions.

## 2. Unambiguous Grammar Reduces Syntax Errors in Generated Code

Every expression starts with an `@intent` marker. The parser never needs lookahead. There is no bracket-matching problem, no indentation ambiguity, no "did I close the `if`?" failure mode.

The grammar is sufficiently constrained that the syntax error space for a code-generating model is narrow: missing a required `@`-keyword or malformed type annotation, not the open-ended class of structural errors possible in Python or JavaScript. This doesn't eliminate errors, but it makes them easier to detect and recover from.

## 3. Structured Agent Collaboration via AST-Level Annotations

`@uncertain` and `@intent` are first-class AST nodes, not comments. The difference matters: a comment can be stripped, misaligned with the code, or ignored by tooling. An AST node can be:
- Queried by selector (`/fn greet/body/@uncertain`)
- Blocked from deployment by a linter rule
- Targeted by a reviewer agent for focused attention

Python and JavaScript can approximate this with structured comments or decorators, but those are conventions — not enforced by the grammar. AVEN makes the distinction between "this is code" and "this code needs review" a parse-time property.

## 4. Capability Security at the Import Boundary

```aven
@use [read] from fs       ; this module can read files, but NOT write
@use [get] from http      ; this module can make GET requests, but NOT POST
```

Every import gates specific capabilities. An agent auditing another agent's code can determine the full side-effect surface from the `@use` declarations alone. Pony and Wuffs have explored similar ideas; AVEN applies the same principle at the module import level, making it immediately visible in the language syntax rather than buried in a type signature.

In autonomous agent pipelines where generated code is executed without human review, this is load-bearing — not decorative.

## 5. Designed for the Reader, Not the Writer

Human languages optimize for the typing experience: abbreviations, syntactic sugar, implicit behavior. AVEN makes the opposite tradeoff: sigil-heavy, explicit effects, no implicit returns, no exceptions.

In agent workflows, code is read and patched far more often than it is written from scratch. The tradeoff — hostile to human authors, legible to automated readers — is deliberate.

The effect arrow system exemplifies this: three orthogonal flags (`?` err, `!` IO, `~` async) compose into 8 precisely distinct arrows. An agent reading a type signature knows exactly which effects are present and which are absent — not "at least IO" but "IO and not async and not err". This precision matters when an auditing agent is deciding whether to allow a function call in a sandboxed pipeline.
