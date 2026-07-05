# Out-of-Scope Knowledge Base

The `.out-of-scope/` directory in a repo stores persistent records of rejected feature requests.

## Directory structure

```
.out-of-scope/
├── dark-mode.md
├── plugin-system.md
└── graphql-api.md
```

One file per **concept**, not per issue.

## File format

```markdown
# Dark Mode

This project does not support dark mode or user-facing theming.

## Why this is out of scope

The rendering pipeline assumes a single color palette defined in
`ThemeConfig`. Supporting multiple themes would require:

- A theme context provider wrapping the entire component tree
- Per-component theme-aware style resolution
- A persistence layer for user theme preferences

This is a significant architectural change that doesn't align with the
project's focus on content authoring.

## Prior requests

- #42 — "Add dark mode support"
- #87 — "Night theme for accessibility"
```

### Naming the file

Use a short, descriptive kebab-case name: `dark-mode.md`, `plugin-system.md`.

### Writing the reason

The reason should be substantive — not "we don't want this" but why. Good reasons reference:

- Project scope or philosophy
- Technical constraints
- Strategic decisions

## When to check `.out-of-scope/`

During triage, read all files in `.out-of-scope/`. Check if the request matches an existing out-of-scope concept by concept similarity, not keyword.

The maintainer may:
- **Confirm** — append to the existing file's "Prior requests", then close
- **Reconsider** — delete or update the out-of-scope file, proceed with normal triage
- **Disagree** — the issues are related but distinct, proceed with normal triage

## When to write to `.out-of-scope/`

Only when an **enhancement** (not a bug) is *rejected* as `wontfix`.

Do **not** write here when something is closed because it's **already implemented**.

The flow:
1. Maintainer decides a feature request is out of scope
2. Check if a matching `.out-of-scope/` file already exists
3. If yes: append the new issue to the "Prior requests" list
4. If no: create a new file with concept name, decision, reason, and first prior request
5. Post a comment on the issue linking to the file
6. Close the issue with `wontfix` label

## Updating or removing out-of-scope files

If the maintainer changes their mind about a previously rejected concept:
- Delete the `.out-of-scope/` file
- The skill does not need to reopen old issues
