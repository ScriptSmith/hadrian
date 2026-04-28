/**
 * Lazy-load the KaTeX stylesheet.
 *
 * `katex/dist/katex.min.css` is ~24 KB minified and ships with the main
 * bundle when imported at module level (the original behavior in
 * `Markdown.tsx` / `StreamingMarkdown.tsx`). Most pages — login, settings,
 * dashboards, the conversation sidebar — never render math, so we defer
 * the request until the first markdown component actually mounts.
 *
 * Vite resolves `import("katex/dist/katex.min.css?inline")`-style URLs at
 * build time, but for plain side-effect imports the dynamic import is
 * still emitted as a separate chunk. Calling this multiple times reuses
 * the same cached promise so the network request happens at most once.
 */
let katexCssPromise: Promise<unknown> | null = null;

export function loadKatexCss(): Promise<unknown> {
  if (katexCssPromise === null) {
    katexCssPromise = import("katex/dist/katex.min.css");
  }
  return katexCssPromise;
}
