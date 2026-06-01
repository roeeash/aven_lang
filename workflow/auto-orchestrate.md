# Auto-Orchestrate: Self-Paced Dev Cycle

## Prompt (for /loop)

Read `/Users/roee.ashkenazi/Desktop/aven-lang/ROADMAP.md` to determine current state.

**If no `## Active Stage:` section exists:**
  - The prior stage is complete. Dispatch the **Planner** agent from `workflow/agents/planner.md` to plan the next stage.
  - After planner returns, document the new `## Active Stage:` section and continue to Programmer step.

**If `## Active Stage:` exists (and Programmer hasn't run yet):**
  - Dispatch the **Programmer** agent from `workflow/agents/programmer.md` (initial-implementation block).
  - Continue to Reviewer step.

**If Programmer work is present (diff shows new changes):**
  - Dispatch the **Reviewer** agent from `workflow/agents/reviewer.md`.
  - Wait for verdict (APPROVED or CHANGES REQUESTED).

**If Reviewer says APPROVED:**
  - Run the **Opus quality gate** (Step 3.5 from orchestration.md):
    - Run `git diff HEAD -- src/ tests/` to prepare context.
    - Dispatch Opus reviewer from `workflow/agents/opus-reviewer.md`.
    - If OPUS VERDICT: APPROVED → proceed to Step 5 (Close).
    - If OPUS VERDICT: CHANGES REQUIRED → dispatch Programmer with response-to-review prompt; loop back to Reviewer.

**If Reviewer says CHANGES REQUESTED:**
  - Dispatch Programmer with **response-to-review block** from `workflow/agents/programmer.md` (second block).
  - After programmer responds, re-dispatch Reviewer.

**Step 5 — Close (when both Reviewer + Opus approve):**
  1. Move `## Active Stage:` section to "Completed Stages" area; rename to `### <name> — **Done**`.
  2. Add "Outcome" paragraph.
  3. Strike through completed item in "Immediate next actions".
  4. Update README.md roadmap table if needed.
  5. Append any follow-ups to "Immediate next actions".
  6. Queue build verification (or note QUEUED if no cargo).

**After Close:**
  - If there are more unblocked items in "Immediate next actions", schedule next loop iteration (self-pace).
  - If all stages are done, log "Stage 2 complete" and exit loop.

---

## Usage

```bash
/loop /orchestrate
```

(or with interval if you prefer a specific cadence, e.g. `/loop 5m /orchestrate`)

The loop will self-pace: after each major step (Plan → Implement → Review → Close), it will re-read ROADMAP.md and determine the next action.

---

## Implementation notes

- **State tracking:** All state lives in ROADMAP.md; no persistent file needed. Each loop iteration reads the file to know what step runs next.
- **Agent dispatch:** Use the `Agent()` tool with prompts from `workflow/agents/`.
- **Opus gate:** Only runs after Reviewer approves; hard cap of 3 rounds per stage.
- **Self-pacing:** Remove `/loop` interval (just `/loop`) to let the orchestrator decide when to re-check.
