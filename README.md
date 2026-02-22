# Hadrian Gateway

An open-source AI Gateway that provides a unified OpenAI-compatible API for routing requests to multiple LLM providers. All enterprise features included. Dual-licensed under Apache 2.0 and MIT.

**[Documentation](https://hadriangateway.com/docs)** | **[API Reference](https://hadriangateway.com/api/docs)**

> [!WARNING]
> Hadrian is experimental, alpha, vibe-coded software and is not ready for production use. The API, configuration format, and database schema are subject to breaking changes that will lead to data loss. Hadrian has not undergone a security audit. Do not expose it to untrusted networks or use it to handle sensitive data. We are not accepting pull requests at this time, but [issues](https://github.com/ScriptSmith/hadrian/issues) and [discussions](https://github.com/ScriptSmith/hadrian/discussions) are welcome.

## Why Hadrian?

- **Single binary, single config.** No complex deployments. Works on a Raspberry Pi or global cloud infrastructure.
- **All features included.** Multi-tenancy, SSO, RBAC, guardrails, semantic caching, cost forecasting. Everything is free.
- **Production ready.** Budget enforcement, rate limiting, circuit breakers, fallback chains, observability.
- **Multi-model chat UI.** Compare responses from multiple models side-by-side with 14 interaction modes.
- **Built-in RAG.** OpenAI-compatible Vector Stores API with document processing, chunking, and search.
- **Studio.** Image generation, TTS, transcription, and translation with multi-model execution.

## Quick Start

```bash
# Download and run
cargo install hadrian
hadrian

# Or with Docker
docker run -p 8080:8080 -e OPENROUTER_API_KEY=sk-... ghcr.io/ScriptSmith/hadrian
```

The gateway starts at `http://localhost:8080` with the chat UI. No database required for basic use.

Running without arguments creates `~/.config/hadrian/hadrian.toml` with sensible defaults, uses SQLite, and opens the browser to the chat UI.

## Configuration

```toml
# Minimal -- just add a provider
[providers.openai]
type = "open_ai"
api_key = "${OPENAI_API_KEY}"
```

```toml
# Multiple providers with fallback
[providers.anthropic]
type = "anthropic"
api_key = "${ANTHROPIC_API_KEY}"
fallback_providers = ["openai"]

[providers.openai]
type = "open_ai"
api_key = "${OPENAI_API_KEY}"
```

Supports OpenAI, Anthropic, AWS Bedrock, Google Vertex AI, Azure OpenAI, and any OpenAI-compatible API (OpenRouter, Ollama, etc). See the [provider docs](https://hadriangateway.com/docs/configuration/providers) for details.

## Features

- **Providers** -- OpenAI, Anthropic, Bedrock, Vertex, Azure, plus any OpenAI-compatible API. Fallback chains, circuit breakers, health checks.
- **Multi-tenancy** -- Organizations, teams, projects, users. Scoped providers, budgets, and rate limits at every level.
- **Auth** -- API keys, OIDC/OAuth, per-org SSO, SAML, SCIM, reverse proxy auth, CEL-based RBAC.
- **Guardrails** -- Blocklist, PII detection, content moderation (OpenAI, Bedrock, Azure). Blocking, concurrent, and post-response modes.
- **Caching** -- Exact match and semantic similarity caching with pgvector or Qdrant.
- **Knowledge Bases** -- File upload, text extraction, OCR, chunking, vector search, re-ranking. OpenAI-compatible Vector Stores API.
- **Cost tracking** -- Microcent precision, time-series forecasting, budget enforcement with atomic reservation.
- **Observability** -- Prometheus metrics, OTLP tracing, structured logging, usage export.
- **Web UI** -- Multi-model chat with 14 modes, frontend tools (Python, JS, SQL, charts), MCP support, admin panel.
- **Studio** -- Image generation, text-to-speech, transcription, and translation across providers.

## API

OpenAI-compatible. Point any OpenAI SDK at Hadrian:

```bash
curl http://localhost:8080/api/v1/responses \
  -H "Content-Type: application/json" \
  -H "X-API-Key: gw_live_..." \
  -d '{"model": "anthropic/claude-opus-4-6", "input": "Hello!"}'
```

Interactive API reference available at `/api/docs` when running.

## Deployment

Available as a single binary, Docker image, or Helm chart.

```bash
# Docker Compose (production)
cd deploy && docker compose -f docker-compose.postgres.yml up -d

# Kubernetes (from source)
cd helm/hadrian && helm dependency update && helm install my-gateway .
```

See the [deployment docs](https://hadriangateway.com/docs/deployment) for Docker Compose configurations, Helm chart options, and production recommendations.

## Development

```bash
# Backend
cargo build && cargo test && cargo clippy && cargo +nightly fmt

# Frontend
cd ui && pnpm install && pnpm dev

# E2E tests
cd deploy/tests && pnpm test
```

## License

Dual-licensed under [Apache 2.0](LICENSE-APACHE) and [MIT](LICENSE-MIT).
