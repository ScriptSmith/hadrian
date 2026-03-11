# Documentation

The documentation site is in `docs/` and uses Fumadocs (Next.js-based). It builds to static HTML that can be embedded in the gateway binary or served from a CDN.

Keep docs up-to-date with the code. If code changes are related to docs pages, update them with information users (not developers) need to know. Run `find docs/content -name '*.mdx' | sort` to see current docs pages.

Read https://www.fumadocs.dev/llms.txt before updates to docs pages. Always use as a reference before starting. Quick start: https://www.fumadocs.dev/docs/index.mdx. Note that eg. `/docs/navigation` means fetch `https://www.fumadocs.dev/docs/navigation.mdx`. Fetching from the fumadocs domain requires using curl in bash.

## Build & Development

```bash
cd docs
pnpm install           # Install dependencies
pnpm dev               # Development server at http://localhost:3000
pnpm build             # Build static site to docs/out/
pnpm lint:fix          # Fix lint errors
pnpm format            # Format code
pnpm generate:openapi  # Regenerate API docs from OpenAPI spec
```

## Architecture

- **Static export**: Builds to `docs/out/` for embedding or serving
- **OpenAPI integration**: API reference pages auto-generated from `openapi/hadrian.openapi.json`
- **Storybook embeds**: UI components are embedded via iframe from Storybook for complete style isolation
  - Symlink `docs/public/storybook` → `../../ui/storybook-static`
  - Use `<StoryEmbed storyId="component-name--story" />` in MDX
  - Requires building Storybook before docs: `cd ui && pnpm storybook:build`

## Writing Guidelines

- Start every page with a one-sentence summary of what it covers
- Use active voice, second person, present tense, imperative mood ("Run the command" not "You should run the command")
- Front-load keywords in headings ("Redis Configuration" not "How to Configure Redis")
- Use realistic data in examples ("acme-corp", "production-api-key") not "foo/bar"
- Use the storybook embeds to show component examples
- Code blocks: always specify language, show complete working examples, include expected output
- Keep pages focused — if past 1500 words, consider splitting
- End pages with "Next Steps" linking to related topics
- Run the linter and formatter after making changes
