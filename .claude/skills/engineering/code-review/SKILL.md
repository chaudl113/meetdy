---
name: code-review
description: Review the changes since a fixed point (commit, branch, tag, or merge-base) along two axes — Standards (does the code follow this repo's documented coding standards?) and Spec (does the code match what the originating issue/PRD asked for?). Runs both reviews in parallel sub-agents and reports them side by side. Use when the user wants to review a branch, a PR, work-in-progress changes, or asks to "review since X".
---

Two-axis review of the diff between `HEAD` and a fixed point the user supplies:

- **Standards** — does the code conform to this repo's documented coding standards?
- **Spec** — does the code faithfully implement the originating issue / PRD / spec?

Both axes run as **parallel sub-agents** so they don't pollute each other's context, then this skill aggregates their findings.

The issue tracker should have been provided to you — run `/setup-matt-pocock-skills` if `docs/agents/issue-tracker.md` is missing.

## Process

### 1. Pin the fixed point

Whatever the user said is the fixed point — a commit SHA, branch name, tag, `main`, `HEAD~5`, etc. If they didn't specify one, ask for it.

Capture the diff command once: `git diff <fixed-point>...HEAD` (three-dot, so the comparison is against the merge-base). Also note the list of commits via `git log <fixed-point>..HEAD --oneline`.

Before going further, confirm the fixed point resolves (`git rev-parse <fixed-point>`) and the diff is non-empty.

### 2. Identify the spec source

Look for the originating spec, in this order:

1. Issue references in the commit messages (`#123`, `Closes #45`, etc.) — fetch from the issue tracker.
2. A path the user passed as an argument.
3. A PRD/spec file under `docs/`, `specs/`, or `.scratch/` matching the branch name or feature.
4. If nothing is found, ask the user where the spec is. If they say there isn't one, the **Spec** sub-agent will skip and report "no spec available".

### 3. Identify the standards sources

Anything in the repo that documents how code should be written, such as `CODING_STANDARDS.md` or `CONTRIBUTING.md`.

On top of whatever the repo documents, the Standards axis always carries the **smell baseline** — Fowler code smells (_Refactoring_, ch.3):

- **Mysterious Name** — rename it; if no honest name comes, the design's murky.
- **Duplicated Code** — extract the shared shape, call it from both.
- **Feature Envy** — move the method onto the data it envies.
- **Data Clumps** — bundle same fields into one type.
- **Primitive Obsession** — give the concept its own small type.
- **Repeated Switches** — replace with polymorphism, or one map both sites share.
- **Shotgun Surgery** — gather what changes together into one module.
- **Divergent Change** — split so each module changes for one reason.
- **Speculative Generality** — delete it; inline back until a real need shows.
- **Message Chains** — hide the walk behind one method.
- **Middle Man** — cut it, call the real target direct.
- **Refused Bequest** — drop the inheritance, use composition.

### 4. Spawn both sub-agents in parallel

Send a single message with two `Agent` tool calls. Use the `general-purpose` subagent for both.

**Standards sub-agent prompt** — include the full diff, standards sources, and the smell baseline. Brief: "Report per file/hunk where the diff violates documented standards or baseline smells. Under 400 words."

**Spec sub-agent prompt** — include the diff and spec. Brief: "Report (a) missing requirements, (b) scope creep, (c) requirements that look wrong. Quote the spec line for each finding. Under 400 words."

### 5. Aggregate

Present the two reports under `## Standards` and `## Spec` headings, verbatim or lightly cleaned.

End with a one-line summary: total findings per axis, and the worst issue _within each axis_ (if any).

## Why two axes

A change can pass one axis and fail the other:

- Code that follows every standard but implements the wrong thing → **Standards pass, Spec fail.**
- Code that does exactly what the issue asked but breaks the project's conventions → **Spec pass, Standards fail.**

Reporting them separately stops one axis from masking the other.
