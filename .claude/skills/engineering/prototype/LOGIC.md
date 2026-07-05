# Logic Prototype

A tiny interactive terminal app that lets the user drive a state model by hand. Use this when the question is about **business logic, state transitions, or data shape** — the kind of thing that looks reasonable on paper but only feels wrong once you push it through real cases.

## When this is the right shape

- "I'm not sure if this state machine handles the edge case where X then Y."
- "Does this data model actually let me represent the case where..."
- "I want to feel out what the API should look like before writing it."

If the question is "what should this look like" — wrong branch. Use [UI.md](UI.md).

## Process

### 1. State the question

Before writing code, write down what state model and what question you're prototyping. One paragraph, at the top of the file.

### 2. Pick the language

Use whatever the host project uses. If the project has no obvious runtime (e.g. a docs repo), ask.

### 3. Isolate the logic in a portable module

Put the actual logic behind a small, pure interface:

- **A pure reducer** — `(state, action) => state`
- **A state machine** — explicit states and transitions
- **A small set of pure functions** over a plain data type
- **A class or module** when the logic genuinely owns ongoing internal state

Keep it pure: no I/O, no terminal code, no `console.log` for control flow. The TUI imports it and calls into it; nothing flows the other direction.

### 4. Build the smallest TUI that exposes the state

Build it as a **lightweight TUI** — on every tick, clear the screen and re-render the whole frame:

1. **Current state**, pretty-printed and diff-friendly. Use **bold** for field names, **dim** for less important context. Native ANSI escape codes are fine.
2. **Keyboard shortcuts**, listed at the bottom: `[a] add user  [d] delete user  [t] tick clock  [q] quit`.

Behaviour: initialise state → read one keystroke → dispatch → re-render → loop until quit.

### 5. Make it runnable in one command

Add a script to the project's existing task runner (`package.json` scripts, `Makefile`, etc).

### 6. Hand it over

Give the user the run command. They'll drive it themselves; the interesting moments are when they say "wait, that shouldn't be possible."

### 7. Capture the answer

When the prototype has done its job, the answer to the question is the only thing worth keeping. Leave a `NOTES.md` next to the prototype.

## Anti-patterns

- **Don't add tests.** A prototype that needs tests is no longer a prototype.
- **Don't wire it to the real database.** Use an in-memory store.
- **Don't generalise.** The prototype answers one question.
- **Don't blur the logic and the TUI together.** Keep the TUI as a thin shell over a pure module.
- **Don't ship the TUI shell into production.** The logic module behind it is the bit worth keeping.
