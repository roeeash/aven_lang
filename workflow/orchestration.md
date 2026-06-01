# Orchestration Playbook

Step-by-step instructions for running one cycle of the 3-agent workflow. The orchestrator is whoever is dispatching agents — typically a higher-tier model in conversation with the user, or a human running this manually.

Each step lists what to do, the prompt to use (referenced from `agents/`), and the success criterion before moving on.

---

## Step 0 — Pre-flight

Read `/Users/roee.ashkenazi/Desktop/AVEN/ROADMAP.md` end-to-end. Confirm:

- There is an `## Immediate next actions` list with at least one queued, unblocked item.
- There is no `## Active Stage:` section already open (if there is, finish that cycle first).
- The seed source compiles in your mind (brace balance ok, no obvious half-finished refactor).

If anything's wrong, fix it before starting a cycle.

---

## Step 1 — Plan

Dispatch the **Planner** agent on the `haiku` model using the prompt template at `agents/planner.md`. The agent reads `ROADMAP.md` and the seed source, picks the next stage, and writes a `## Active Stage: <name>` section.

**Concrete dispatch syntax** (pick the one matching your session):

```
# Claude Code:
Task(
  subagent_type: "general-purpose",
  model: "haiku",
  description: "Planner — pick + plan next stage",
  prompt: <the prompt block from agents/planner.md, with {REPO_ROOT}
           substituted for /Users/roee.ashkenazi/Desktop/AVEN>
)

# Cowork mode:
Agent(
  subagent_type: "general-purpose",
  model: "haiku",
  description: "Planner — pick + plan next stage",
  prompt: <same as above>
)
```

The prompt block is the content between the triple backticks in `agents/planner.md`. Substitute `{REPO_ROOT}` everywhere it appears. Also tell the planner whether `cargo` is available in its environment — that constraint matters for stage selection.

**After the planner returns, before dispatching the programmer:**

1. Read the new `## Active Stage` section. Sanity-check its scope.
2. **Diff the seed source against your mental model** — the planner may have shipped pre-work in `.rs` files despite the no-code guardrail. If it did:
   - If the code is clean and in-scope for the active stage: keep it, add a `Workflow note (orchestrator)` block right under the Active Stage heading explaining what the planner did outside its role. Mention this in your final report.
   - If the code is buggy or out of scope: roll it back via `git checkout -- <files>` (or equivalent) and dispatch the planner again with even sharper guardrails.

**Success criterion:** `## Active Stage` section exists in `ROADMAP.md` with all six sub-sections (Goal / Files touched / Specific changes / Tests to add / Definition of done / Out of scope). Source either untouched, or planner pre-work accepted with deviation note.

---

## Step 2 — Implement

Dispatch the **Programmer** agent using `agents/programmer.md` (initial-implementation prompt block, NOT the response-to-review block). Same `Task`/`Agent` dispatch shape as Step 1; only the `prompt` field changes. The agent reads the Active Stage section and implements it exactly.

**Success criterion:** The agent reports brace balance = 0 across every `.rs` file, the count of `Expr::` match arms in `eval.rs` equals the number of `Expr` variants in `ast.rs` (or the analog for whatever stage this is), and the planned tests are present by name. Read the agent's reported diff summary; if it claims it modified files that weren't in the plan's Files-touched list, push back before going to review.

---

## Step 3 — Review

Dispatch the **Reviewer** agent using `agents/reviewer.md`. Same dispatch shape as Steps 1–2; only the prompt changes. The agent reads the Active Stage, the spec, and the programmer's output.

**The agent's reply MUST end with one of:**

- `VERDICT: APPROVED` → go to Step 5.
- `VERDICT: CHANGES REQUESTED` followed by a numbered list (max 6) of file:line-cited concerns → go to Step 4.

If the reviewer rambles without a verdict, or cites concerns without file:line, dispatch it again with a sharper prompt.

---

## Step 3.5 — Opus quality gate

After the haiku reviewer emits `VERDICT: APPROVED`, run the Opus quality gate before closing.

**Prepare context (orchestrator runs these):**

```bash
cd /Users/roee.ashkenazi/Desktop/AVEN/seed && git diff HEAD -- src/ tests/
```

Also read the `## Active Stage` section from `ROADMAP.md` and identify which `AVEN_SPEC.md` sections are relevant (listed in the stage plan).

**Dispatch the Opus Reviewer** using `agents/opus-reviewer.md`. Model: `opus` (claude-opus-4-7). Substitute `{ACTIVE_STAGE_SECTION}`, `{SPEC_SECTIONS}`, and `{GIT_DIFF}` into the prompt template.

**After Opus returns:**

1. Append the full Opus reply verbatim to `workflow/opus-review-log.md` under a new heading `## <Stage name> — <date> / Round <N>`.
2. If `OPUS VERDICT: APPROVED` → go to Step 5.
3. If `OPUS VERDICT: CHANGES REQUIRED`:
   - Dispatch the Programmer with the **response-to-review prompt** from `programmer.md`, pasting the Opus verdict into the `----- VERDICT VERBATIM -----` placeholder.
   - After the programmer responds, re-dispatch the haiku Reviewer (Step 3), then re-run Opus (this step).
   - **Hard cap: 3 Opus rounds total.** If still `CHANGES REQUIRED` at round 3: escalate to the user. Do not close the stage with unresolved Opus findings unless the programmer's pushback is unambiguously correct on the merits (trace required).

**Success criterion:** `OPUS VERDICT: APPROVED` received within 3 rounds.

---

## Step 4 — Respond and re-review

Dispatch the **Programmer** agent again — this time with the **response-to-review prompt block** from `agents/programmer.md` (the second triple-backtick block in that file, not the first). Paste the reviewer's full reply verbatim into the placeholder marked `----- VERDICT VERBATIM -----` inside the prompt. The agent either fixes each concern or pushes back with a trace.

After the programmer responds, **dispatch the Reviewer again** for a second review pass.

**Hard cap: 2 review rounds total** (one initial + one re-review). If the reviewer is still requesting changes after the cap:

- If the programmer's pushback against the remaining concerns is convincing (the reviewer is wrong on the merits, with a trace to prove it): the orchestrator closes the loop, marks the stage as Done, and documents the disagreement under the Completed Stages entry.
- If the programmer agrees with the remaining concerns but couldn't fix them: the orchestrator either (a) finishes the fixes manually and closes the loop, or (b) re-queues the stage as a new Immediate next action with a "carryover from [previous attempt]" note.

**If a reviewer raises an issue that's clearly out of scope** (style nit, "consider also" suggestion, future feature): the orchestrator can ignore it and document.

---

## Step 5 — Close

1. **Move the Active Stage section** from its current spot to the "Completed Stages" area of `ROADMAP.md`. Rename it from `## Active Stage: <name>` to `### <name> — **Done**`. Add an "Outcome" paragraph summarizing what landed and any workflow notes (planner overreach, reviewer false positives, etc.).
2. **Strike through the corresponding entry** in "Immediate next actions": `~~Original text.~~ **Done.** Tests: x, y, z.`
3. **Append any follow-ups** the reviewer flagged or the programmer surfaced as new entries in "Immediate next actions". Order them by leverage.
4. **Note the build status.** If you ran `cargo` to verify, write the exact tests-passing count. If you didn't (sandbox), write that and queue re-verification as the next action.
5. Update any relevant status lines in `README.md`'s roadmap table.

---

## Failure modes and how to handle them

**Planner picks a stage that requires `cargo` even though the sandbox has no toolchain.** Reject the plan and re-dispatch the planner with the constraint highlighted.

**Programmer claims tests pass without running them.** Their report should distinguish between syntactic checks (brace balance, grep) and `cargo test` results. If it claims tests pass without saying which mechanism it used, treat that as unverified.

**Reviewer comes back with style nits.** Filter them out before passing to the programmer. The programmer's time is better spent on real issues.

**Programmer's pushback is wrong (claims a bug isn't a bug when it is).** Orchestrator overrides, dispatches programmer again with an explicit "the reviewer is correct, apply the fix" framing.

**Two review rounds and there's still a real, unresolved bug.** Pause the workflow. Fix it manually or escalate to the user. Don't mark the stage Done with known defects.
