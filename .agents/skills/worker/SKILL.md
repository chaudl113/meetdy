---
name: worker
description: Execute beads autonomously within a track. Handles bead-to-bead context persistence via Agent Mail, uses preferred tools from AGENTS.md, and reports progress to orchestrator.
---

# Worker Skill: Autonomous Bead Execution

Executes beads within an assigned track, maintaining context via Agent Mail.

## Your Authority (Read First)

You are a **sub-agent** executing a bounded track under an orchestrator. Your authority is limited to your assigned beads and file scope.

**You may:**

- Implement your beads using any tools available
- Make local implementation decisions within your file scope
- Report blockers, propose changes, and ask questions via Agent Mail

**You may NOT:**

- Change architecture, interfaces, or project direction
- Modify files outside your reserved scope
- Re-scope, split, skip, or add beads
- Close a bead without passing verification (see Step 3)

If a bead seems wrong, impossible, or would require exceeding your authority: **stop and report to the orchestrator**. Do not improvise beyond scope. Deviating "helpfully" is the most common way sub-agents degrade output quality.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  TRACK LOOP (repeat for each bead in track)                                 │
│                                                                             │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐                    │
│  │ START BEAD   │ → │ WORK ON BEAD │ → │ COMPLETE     │ ──┐                │
│  │              │   │              │   │ BEAD         │   │                │
│  │ • Read ctx   │   │ • Implement  │   │ • Verify     │   │                │
│  │ • Claim      │   │ • Use tools  │   │ • Report     │   │                │
│  │              │   │ • Check mail │   │ • Save ctx   │   │                │
│  └──────────────┘   └──────────────┘   └──────────────┘   │                │
│         ▲                                                  │                │
│         └──────────────────────────────────────────────────┘                │
└─────────────────────────────────────────────────────────────────────────────┘
```

Your work is isolated in a dedicated git worktree assigned by the orchestrator. You never share a working tree with other workers — no file reservation needed.

---

## Initial Setup (Once Per Track)

### 1. Register Agent Identity

**Tool**: `mcp__mcp_agent_mail__register_agent`

| Parameter          | Value                     |
| ------------------ | ------------------------- |
| `project_key`      | `<absolute-project-path>` |
| `program`          | `amp`                     |
| `model`            | `<your-model>`            |
| `task_description` | `Track N: <description>`  |

_Name auto-generated if omitted (e.g., BlueLake)_

### 2. Understand Your Assignment

From orchestrator: **Track number**, **Beads (in order)**, **File scope**, **Worktree path** (your isolated working tree), **Epic thread** (`<epic-id>`), **Track thread** (`track:<AgentName>:<epic-id>`)

All work happens inside your worktree. Other workers have their own worktrees — you will never see their changes and they will never see yours until the orchestrator merges.

---

## Bead Execution Loop

### Step 1: Start Bead

#### 1.1 Read Context from Previous Bead

**Tool**: `mcp__mcp_agent_mail__summarize_thread`

| Parameter          | Value                         |
| ------------------ | ----------------------------- |
| `project_key`      | `<path>`                      |
| `thread_id`        | `track:<AgentName>:<epic-id>` |
| `include_examples` | `true`                        |

#### 1.2 Check Inbox

**Tool**: `mcp__mcp_agent_mail__fetch_inbox`

| Parameter        | Value         |
| ---------------- | ------------- |
| `project_key`    | `<path>`      |
| `agent_name`     | `<AgentName>` |
| `include_bodies` | `true`        |

#### 1.3 Claim Bead

```bash
bd update <bead-id> --status in_progress
bd show <bead-id>
```

---

### Step 2: Work on Bead

#### 2.1 Explore Codebase

**Tool**: `mcp__gkg__search_codebase_definitions`

| Parameter               | Value         |
| ----------------------- | ------------- |
| `search_terms`          | `["<terms>"]` |
| `project_absolute_path` | `<path>`      |

**Tool**: `mcp__gkg__get_references`

| Parameter            | Value      |
| -------------------- | ---------- |
| `absolute_file_path` | `<file>`   |
| `definition_name`    | `<symbol>` |

#### 2.2 Make Changes

**Tool**: `mcp__morph_mcp__edit_file`

| Parameter     | Value                                 |
| ------------- | ------------------------------------- |
| `path`        | `<file>`                              |
| `code_edit`   | Use `// ... existing code ...` syntax |
| `instruction` | `<what you're changing>`              |

After edits: `get_diagnostics("<edited-file>")`

#### 2.3 For UI Work

Load `frontend-design` skill first, then follow this workflow:

##### 2.3.1 Search for Components

**Tool**: `mcp__shadcn__search_items_in_registries`

| Parameter    | Value         |
| ------------ | ------------- |
| `registries` | `["@shadcn"]` |
| `query`      | `<component>` |

##### 2.3.2 View Component Details

**Tool**: `mcp__shadcn__view_items_in_registries`

| Parameter | Value                     |
| --------- | ------------------------- |
| `items`   | `["@shadcn/<component>"]` |

##### 2.3.3 Get Usage Examples

**Tool**: `mcp__shadcn__get_item_examples_from_registries`

| Parameter    | Value              |
| ------------ | ------------------ |
| `registries` | `["@shadcn"]`      |
| `query`      | `<component>-demo` |

##### 2.3.4 Install Component

**Tool**: `mcp__shadcn__get_add_command_for_items`

| Parameter | Value                     |
| --------- | ------------------------- |
| `items`   | `["@shadcn/<component>"]` |

Then run the returned command (e.g., `npx shadcn@latest add button`).

##### 2.3.5 Verify Installation

**Tool**: `mcp__shadcn__get_audit_checklist`

| Parameter | Value                             |
| --------- | --------------------------------- |
| `reason`  | `Verify <component> installation` |

#### 2.4 Check Inbox Periodically

Use `mcp__mcp_agent_mail__fetch_inbox` with `since_ts` parameter.

#### 2.5 If Blocker or Interface Change

See `reference/message-templates.md` for message formats.

---

### Step 3: Complete Bead

#### 3.1 Verify (Proof of Work — MANDATORY)

A bead is NOT done because you believe it is done. It is done when verification passes mechanically.

```bash
get_diagnostics("<project-path>")
bun run check-types
bun run build
```

If the bead is linked to a harness story with a `verify_command`, run it:

```bash
./scripts/bin/harness-cli story verify <story-id>
```

**Hard rule:** if any check fails, you may NOT close the bead. Fix and re-verify, or report a blocker to the orchestrator. Never report COMPLETE on a failing verification.

#### 3.2 Close Bead (with evidence)

```bash
bd close <bead-id> --reason "<concise summary>"
```

Your COMPLETE message (3.3) must include evidence, not just a claim:

- Verification commands run and their results (pass)
- Files changed
- Any deviations from the bead description (should be none — see Your Authority)

#### 3.3 Report to Orchestrator

**Tool**: `mcp__mcp_agent_mail__send_message`

| Parameter     | Value                                |
| ------------- | ------------------------------------ |
| `project_key` | `<path>`                             |
| `sender_name` | `<AgentName>`                        |
| `to`          | `["<OrchestratorName>"]`             |
| `thread_id`   | `<epic-id>`                          |
| `subject`     | `[<bead-id>] COMPLETE`               |
| `body_md`     | See `reference/message-templates.md` |

#### 3.4 Save Context for Next Bead

Self-addressed message to track thread. See `reference/message-templates.md`.

#### 3.5 No Release Needed

File release is not needed — your worktree is isolated. When the bead is done, simply save context and continue. (The orchestrator manages worktree lifecycle.)

---

### Step 4: Continue to Next Bead

Loop back to Step 1. Context from Step 3.4 available via track thread.

---

## Track Completion

When all beads done, send track complete message (see `reference/message-templates.md`), then return:

```
Track N (<AgentName>) Complete:
- Completed beads: a, b, c
- Summary: <what was built>
- All acceptance criteria met
```

---

## Quick Reference

### Bead Lifecycle Checklist

```
START: summarize_thread → fetch_inbox → bd update
WORK:  gkg tools → morph edits → get_diagnostics → check inbox
DONE:  verify (MUST pass) → bd close → send_message (orchestrator, with evidence) → send_message (self)
NEXT:  loop to START
```

### Thread Reference

| Thread                        | Purpose                                 |
| ----------------------------- | --------------------------------------- |
| `<epic-id>`                   | Cross-agent, orchestrator communication |
| `track:<AgentName>:<epic-id>` | Your personal context persistence       |

### Tool Reference

| Task              | Tool                                             |
| ----------------- | ------------------------------------------------ |
| Find code         | `mcp__gkg__search_codebase_definitions`          |
| Get definition    | `mcp__gkg__get_definition`                       |
| Find usages       | `mcp__gkg__get_references`                       |
| Edit file         | `mcp__morph_mcp__edit_file`                      |
| Search components | `mcp__shadcn__search_items_in_registries`        |
| View components   | `mcp__shadcn__view_items_in_registries`          |
| Get examples      | `mcp__shadcn__get_item_examples_from_registries` |
| Install component | `mcp__shadcn__get_add_command_for_items`         |
| Verify install    | `mcp__shadcn__get_audit_checklist`               |

---

## Additional Resources

- **Message Templates**: `reference/message-templates.md` for all Agent Mail message formats
