# UI Prototype

Generate **several radically different UI variations** on a single route, switchable from a floating bottom bar. The user flips between variants in the browser, picks one (or steals bits from each), then throws the rest away.

If the question is about logic/state rather than what something looks like — wrong branch. Use [LOGIC.md](LOGIC.md).

## When this is the right shape

- "What should this page look like?"
- "I want to see a few options for this dashboard before committing."
- "Try a different layout for the settings screen."
- Any time the user would otherwise spend a day picking between three vague mockups in their head.

## Two sub-shapes — strongly prefer sub-shape A

### Sub-shape A — adjustment to an existing page (preferred)

The route already exists. Variants are rendered **on the same route**, gated by a `?variant=` URL search param. The existing data fetching, params, and auth all stay — only the rendering swaps.

### Sub-shape B — a new page (last resort)

Only use this when the thing being prototyped genuinely has no existing page to live inside.

## Process

### 1. State the question and pick N

Default to **3 variants**. More than 5 stops being radically different and starts being noise.

Write down the plan in one line:

> "Three variants of the settings page, switchable via `?variant=`, on the existing `/settings` route."

### 2. Generate radically different variants

Draft each variant. Hold each one to:

- The page's purpose and the data it has access to.
- The project's component library / styling system (TailwindCSS, shadcn, MUI, plain CSS, whatever).
- A clear exported component name, e.g. `VariantA`, `VariantB`, `VariantC`.

Variants must be **structurally different** — different layout, different information hierarchy, different primary affordance, not just different colours.

### 3. Wire them together

Create a single switcher component on the route:

```tsx
// pseudo-code — adapt to the project's framework
const variant = searchParams.get('variant') ?? 'A';
return (
  <>
    {variant === 'A' && <VariantA {...data} />}
    {variant === 'B' && <VariantB {...data} />}
    {variant === 'C' && <VariantC {...data} />}
    <PrototypeSwitcher variants={['A','B','C']} current={variant} />
  </>
);
```

### 4. Build the floating switcher

A small fixed-position bar at the bottom-centre of the screen with:

- **Left arrow** — cycles to the previous variant (wraps around).
- **Variant label** — shows the current variant key and name.
- **Right arrow** — cycles forward (wraps around).

Behaviour:

- Clicking an arrow updates the URL search param.
- Keyboard: `←` and `→` arrow keys also cycle. Don't intercept when `<input>`, `<textarea>`, or `[contenteditable]` is focused.
- Visually distinct from the page (e.g. high-contrast pill, subtle shadow).
- Hidden in production builds — gate on `process.env.NODE_ENV !== 'production'`.

### 5. Hand it over

Surface the URL (and the `?variant=` keys). The user will flip through.

### 6. Capture the answer and clean up

Once a variant has won, write down which one and why. Then delete losing variants and the switcher; fold the winner into the existing page.

## Anti-patterns

- **Variants that differ only in colour or copy.** Real variants disagree about structure.
- **Sharing too much code between variants.** Each variant should be free to throw out the layout.
- **Wiring variants to real mutations.** Read-only prototypes are fine.
- **Promoting the prototype directly to production.** Rewrite it properly when you fold it in.
