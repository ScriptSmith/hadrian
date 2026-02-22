# OpenAPI Specs

## Hadrian (generated)

| Filename | How to regenerate |
|----------|-------------------|
| [hadrian.openapi.json](./hadrian.openapi.json) | `cargo run -- openapi > openapi/hadrian.openapi.json` |

This is the only spec checked into the repository. It is generated from the Hadrian codebase.

## Third-party reference specs (fetched on demand)

These specs are **not** checked into the repository. Fetch them with:

```bash
./scripts/fetch-openapi-specs.sh        # all specs
./scripts/fetch-openapi-specs.sh openai # just OpenAI
```

| Provider | Filename | Source |
|----------|----------|--------|
| OpenAI | `openai.openapi.json` / `openai.openapi.yml` | [openai/openai-openapi](https://github.com/openai/openai-openapi) (MIT) |
| Anthropic | `anthropic.openapi.json` | [anthropics/anthropic-sdk-typescript](https://github.com/anthropics/anthropic-sdk-typescript) |
| OpenRouter | `openrouter.openapi.yml` | [openrouter.ai](https://openrouter.ai/docs/) |

### What uses them

- **`openai.openapi.json`** — Used by [`scripts/openapi-conformance.py`](../scripts/openapi-conformance.py) for API conformance checking, and by `src/validation/schema.rs` for runtime response validation.
- **`anthropic.openapi.json`** / **`openrouter.openapi.yml`** — Used as reference when implementing provider support. Not consumed by any build or CI step.
