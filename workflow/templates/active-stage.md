# Active Stage Template

The Planner copies this skeleton into `ROADMAP.md` (appended after the "Immediate next actions" list) and fills in every section. The headings are exact and other agents grep for them.

---

```markdown
## Active Stage: <short imperative name>

<!--
Optional: if the orchestrator accepted any pre-work that the planner shipped
outside its role, document it here BEFORE the Goal section:

> **Workflow note (orchestrator).** The planner agent modified <files> outside
> its role. The work is <accepted | rejected and rolled back>. The plan below
> describes the <remaining | full> work.
-->

**Goal**

<One paragraph. What changes, why, what milestone it unblocks. Be specific
about the value — "X is needed by M2 type errors" beats "improves the
codebase". Reference `plan.md` or `AVEN_SPEC.md` sections by number when they
constrain the design.>

**Files touched**

- `seed/src/<file>.rs` — <one-line summary>
- `seed/src/<file>.rs` — <one-line summary>
- `seed/tests/<file>.rs` — <one-line summary>

**Specific changes**

- **<file>**: <Per-file bulleted changes. Be precise enough that a different
  agent can implement without re-deriving the design. Cite line numbers and
  function names from the current source. State the new function signatures
  or enum variants verbatim where possible.>
- **<file>**: <...>

**Tests to add**

- `<test_name>` (in `<file>`) — <what it verifies, including the specific
  assertion>.
- `<test_name>` (in `<file>`) — <...>

**Definition of done**

- <Checkable criterion 1. e.g., "All N existing tests still pass syntactically
  (brace balance preserved)".>
- <Checkable criterion 2. e.g., "Every Expr variant except Nil has a
  SourceSpan field, verified by grep".>
- <Checkable criterion 3.>

**Out of scope**

- <Adjacent feature deliberately deferred. e.g., "Printing spans in error
  messages — deferred to M7 polish".>
- <Another deferred thing.>
```

---

## Constraints the planner must satisfy

- **Length.** Aim for under 80 lines in the rendered section. The Programmer reads on a token budget.
- **Tests by name.** Every test the plan promises must be named explicitly. The Reviewer greps for those names.
- **Files touched is exhaustive.** If the Programmer ends up needing to edit a file not in this list, that's a planning failure — the planner should re-plan rather than the programmer should improvise.
- **Out of scope is real.** Things genuinely deferred go here. Don't pad with irrelevant items.
