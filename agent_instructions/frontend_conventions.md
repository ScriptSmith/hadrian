# Frontend Conventions

## General

- Run `./scripts/generate-openapi.sh` to generate the OpenAPI client
- Use React Query for all API calls (via generated hey-api client)
- Components are in `ui/src/components/` with PascalCase directories
- Pages and large components should be broken down into multiple components
- Each component must have a `.stories.tsx` file for Storybook
- Prefer Tailwind utility classes over custom CSS

## Accessibility (WCAG 2.1 AA)

All UI components must meet WCAG 2.1 AA standards. Two tools enforce this automatically:

- **`eslint-plugin-jsx-a11y`** — Static linting (runs with `pnpm lint`). Catches missing labels, invalid ARIA attributes, etc.
- **`@storybook/addon-a11y`** — Runtime axe-core testing (runs with `pnpm test-storybook`). Set to `error` mode in `ui/.storybook/preview.ts` — all story files must pass.

When writing new components:
- Add `aria-label` to icon-only buttons (e.g., `aria-label="Copy code"`)
- Associate form controls with labels (`useId()` + `htmlFor`, or `aria-label` for switches/toggles)
- Use theme CSS variables for text colors — don't hard-code Tailwind colors below `-700` (light) or above `-400` (dark) on white/dark backgrounds
- Don't reduce text opacity (no `/60`, `/70`, `/80` suffixes on `text-muted-foreground`)
- Add `sr-only` text for empty table headers (action columns) and visually hidden labels
- Add `tabIndex={0}` to scrollable containers that aren't natively focusable
- For Storybook false positives (landmark nesting, heading order in isolation), suppress per-story via `parameters.a11y.config.rules` — never disable globally
