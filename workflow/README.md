# AVEN Stage Workflow

A reusable 3-agent loop for moving an AVEN seed-compiler stage from "queued in `ROADMAP.md`'s Immediate next actions" to "Done in Completed Stages." All three agents run on the **haiku** model; an orchestrator (a higher-tier model, typically the user's main session) sequences them and arbitrates.

## How to run a cycle

This workflow is conversation-driven by design — there is no standalone runner script. The orchestrator needs to read deviations, arbitrate disagreements, and decide when to close vs. iterate, which is not script-shaped work.

### Quickest path: just say the magic phrase

In any orchestrator session (Cowork mode, Claude Code, or the API console), say:

> Run the next workflow cycle.

The orchestrator reads `MEMORY.md` (or `CLAUDE.md`), follows the directives there into `orchestration.md`, and runs the cycle.

### If you're an orchestrator that just got pointed at this repo

You're reading this either because (a) the user told you to advance the work and you're discovering the workflow now, or (b) your memory file pointed you here. Either way:

**The bootstrap prompt (paste into your own context if useful):**

> I am the orchestrator of the AVEN 3-agent workflow. My job for this cycle: (1) read `workflow/orchestration.md` — Steps 0–5; (2) read the three agent prompt templates in `workflow/agents/`; (3) dispatch Planner → Programmer → Reviewer as Haiku sub-agents using my session's sub-agent dispatch tool (`Task` in Claude Code, `Agent` in Cowork mode), passing `{REPO_ROOT}` = the absolute path to this repo; (4) loop on review at most twice, then arbitrate; (5) close the stage in `ROADMAP.md`. I do not implement seed changes myself — I dispatch the Programmer sub-agent.

### Tool mapping for orchestrators

| If your session has... | Use it like this |
|---|---|
| Claude Code with the `Task` tool | `Task(subagent_type="general-purpose", model="haiku", prompt=<agent template>)` |
| Cowork mode with the `Agent` tool | `Agent(subagent_type="general-purpose", model="haiku", prompt=<agent template>)` |
| Direct Anthropic API access | POST to `claude-haiku-4-5` with the agent template as the message |

The role separation matters more than the specific model. Use whatever sub-agent dispatch you have available; just make sure each agent runs on Haiku (cheap + fast + doesn't second-guess the planner's choice).

### Manual fallback

If you have no sub-agent dispatch at all, you can still drive this manually: open each agent file, copy the prompt block inside the triple backticks, send to a Haiku endpoint yourself, apply the output (Planner → ROADMAP.md, Programmer → seed/, Reviewer → reports inline), advance. `orchestration.md` is the playbook either way.

## When to use this

Any time the next planned change to the seed (`/seed/`) is well-scoped enough to be described in a single Active Stage section. Examples that fit: adding a new lexer token family, wiring an AST variant through parser + eval + tests, cleaning a class of dead-code warnings. Examples that don't fit: open-ended exploration, design decisions, anything requiring a `cargo` run inside an offline sandbox.

## Directory layout

```
workflow/
├── README.md                  # this file
├── orchestration.md           # step-by-step playbook for running one cycle
├── agents/
│   ├── planner.md             # role + prompt template + guardrails
│   ├── programmer.md          #   "
│   └── reviewer.md            #   "
└── templates/
    └── active-stage.md        # skeleton the planner writes into ROADMAP.md
```

## The loop in one paragraph

The **Planner** reads `ROADMAP.md` and the seed source, picks the highest-leverage queued action, and writes a tight `## Active Stage: <name>` section into `ROADMAP.md` with goal / files touched / specific changes / tests to add / definition of done / out of scope. The **Programmer** reads that section and the source, implements the plan literally, runs syntactic checks, and reports a diff summary. The **Reviewer** reads the Active Stage, the spec, and the source after the programmer's pass, and emits `VERDICT: APPROVED` or `VERDICT: CHANGES REQUESTED` with up to 6 file:line-cited concerns. If concerns: the Programmer either fixes each or pushes back with a trace. Cap at **2 review rounds total**; after that the orchestrator decides and documents. On close, the stage moves to "Completed Stages" and follow-ups become new "Immediate next actions."

## Quick start for an orchestrator

1. Open `orchestration.md`. It contains the verbatim dispatch prompts to copy-paste into agent calls, plus break-loop conditions.
2. Spawn the Planner agent with `agents/planner.md`'s prompt template, filled in with current state.
3. After the planner writes the Active Stage into `ROADMAP.md`, **read it** before dispatching the programmer — verify the planner didn't overreach into code. (Observed failure mode: planner shipping pre-work in `.rs` files. Acceptable only if clean and in-scope; document the deviation.)
4. Spawn Programmer with `agents/programmer.md`.
5. Spawn Reviewer with `agents/reviewer.md`.
6. If `CHANGES REQUESTED`, re-spawn Programmer with the review verbatim. Then re-spawn Reviewer. Stop after two review rounds.
7. Move the Active Stage to Completed Stages in `ROADMAP.md`; queue follow-ups.

## Lessons baked into the templates

These come from the first execution of this workflow (the "Add source spans to AST nodes" stage). Each is encoded as an explicit rule in the relevant agent template:

- **Planner has a tendency to write code despite being told not to.** Every planner prompt now includes: *"If you find yourself opening Write or Edit on a `.rs` file, stop and rewrite the plan instead."* If the planner ships code anyway and the orchestrator keeps it, the deviation is documented in the ROADMAP Active Stage section.
- **Reviewer false positives happen.** Reviewers must cite file:line for each concern. Programmers may push back with a trace; in those cases the orchestrator may close without a re-review. Style suggestions are out of scope for reviews.
- **No `cargo` invocations inside the sandbox.** Verification uses brace balance, grep cross-checks, and structural sanity. Real build/test verification happens on a host with a toolchain — that's the user's job, queued as the first immediate next action whenever the toolchain claim hasn't been re-verified.
