"use client";

import { useEffect, useRef } from "react";
import Link from "next/link";
import { Server, Shield, Users, Zap, Eye, Code, Brain } from "lucide-react";
import { StoryEmbed } from "@/components/story-embed";
import { QuickStartSelector } from "@/components/quick-start-selector";

function GitHubIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden="true">
      <path d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0 1 12 6.844a9.59 9.59 0 0 1 2.504.337c1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.02 10.02 0 0 0 22 12.017C22 6.484 17.522 2 12 2Z" />
    </svg>
  );
}

// --- See it in Action (Gallery) ---

const demos = [
  {
    id: "knowledge-bases",
    title: "Knowledge Bases",
    description: "Search uploaded documents with vector search, citations, and inline references.",
    storyId: "chat-chatview--knowledge-bases",
  },
  {
    id: "chat",
    title: "Multi-Model Chat",
    description:
      "Compare responses from multiple models side-by-side with advanced multi-model modes.",
    storyId: "chat-chatview--multi-model-conversation",
  },
  {
    id: "execute-code",
    title: "Execute Code",
    description: "Run Python in the browser and display interactive chart artifacts inline.",
    storyId: "chat-chatview--execute-code",
  },
  {
    id: "studio",
    title: "Studio",
    description: "Generate images across providers simultaneously with cost tracking.",
    storyId: "pages-studiopage--images",
  },
  {
    id: "usage",
    title: "Usage Dashboard",
    description: "Track costs per user, team, and project with microcent precision.",
    storyId: "components-usagedashboard--organization",
  },
];

function DemoGallery() {
  const chatRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = chatRef.current;
    const container = el?.parentElement;
    if (el && container) {
      const scrollLeft =
        el.offsetLeft - container.offsetLeft - (container.clientWidth - el.offsetWidth) / 2;
      container.scrollLeft = scrollLeft;
    }
  }, []);

  return (
    <div className="flex snap-x snap-mandatory gap-6 overflow-x-auto pb-4">
      <div className="w-[15%] shrink-0" aria-hidden="true" />
      {demos.map((demo) => (
        <div
          key={demo.id}
          ref={demo.id === "chat" ? chatRef : undefined}
          className="w-[70%] shrink-0 snap-center"
        >
          <h3 className="mb-1 text-lg font-semibold">{demo.title}</h3>
          <p className="mb-3 text-sm text-fd-muted-foreground">{demo.description}</p>
          <div className="overflow-hidden rounded-xl border border-fd-border shadow-lg">
            <StoryEmbed storyId={demo.storyId} height={850} />
          </div>
        </div>
      ))}
      <div className="w-[15%] shrink-0" aria-hidden="true" />
    </div>
  );
}

// --- Providers ---

const providers = [
  {
    name: "OpenAI",
    description: "GPT-5.3, o3, DALL-E, TTS, Whisper, embeddings",
  },
  {
    name: "Anthropic",
    description: "Claude Opus 4.6, Sonnet 4.5, Haiku 4.5 with prompt caching",
  },
  {
    name: "AWS Bedrock",
    description: "Claude, Nova, Llama via AWS IAM auth",
  },
  {
    name: "Google Vertex AI",
    description: "Gemini 2.5 models via GCP service accounts",
  },
  {
    name: "Azure OpenAI",
    description: "OpenAI models on Azure infrastructure",
  },
  {
    name: "OpenAI Compatible",
    description:
      "OpenRouter, Ollama, Together AI, Groq, Fireworks AI, vLLM, LM Studio, Cerebras, DeepSeek, Mistral, Cohere, Perplexity, and more",
  },
];

// --- Everything Included ---

const featureCategories = [
  {
    icon: Server,
    title: "Infrastructure",
    items: [
      "Single binary, single config file",
      "SQLite, Postgres, or stateless",
      "Redis caching",
      "S3-compatible storage",
      "Provider fallbacks & health checks",
      "Helm chart for Kubernetes",
    ],
  },
  {
    icon: Brain,
    title: "AI Capabilities",
    items: [
      "Multi-model chat",
      "Image generation",
      "TTS & transcription",
      "Knowledge bases / RAG",
      "Model catalog",
    ],
  },
  {
    icon: Shield,
    title: "Security & Auth",
    items: [
      "API keys & service accounts",
      "OIDC / OAuth / SAML SSO",
      "CEL-based RBAC",
      "Guardrails & content moderation",
      "Rate limiting",
    ],
  },
  {
    icon: Users,
    title: "Multi-tenancy",
    items: [
      "Organizations, teams, projects",
      "Dynamic providers",
      "Scoped budgets",
      "Per-tenant SSO",
      "SCIM provisioning",
    ],
  },
  {
    icon: Eye,
    title: "Observability",
    items: [
      "Usage & cost tracking",
      "Cost forecasting",
      "Prometheus metrics",
      "OpenTelemetry tracing",
      "Audit logs & SIEM",
    ],
  },
  {
    icon: Code,
    title: "Developer Experience",
    items: [
      "OpenAI-compatible API",
      "OpenAPI docs & Scalar UI",
      "MCP servers",
      "Frontend tools (Python/JS/SQL/Charts)",
      "Web UI with admin panel",
    ],
  },
];

// --- Page ---

export default function HomePage() {
  return (
    <div className="flex flex-col">
      {/* Hero */}
      <section className="relative overflow-hidden border-b bg-gradient-to-b from-fd-background to-fd-muted/30 py-16 md:py-24">
        <div className="mx-auto max-w-6xl px-4">
          <div className="text-center">
            <h1 className="mb-6 text-4xl font-bold tracking-tight md:text-6xl">Hadrian Gateway</h1>
            <p className="mx-auto mb-0 max-w-2xl text-lg text-fd-muted-foreground md:text-xl">
              A unified AI gateway with every enterprise feature included.
            </p>
            <p className="mx-auto mb-8 max-w-2xl text-lg text-fd-muted-foreground md:text-xl">
              Completely Open source and free.
            </p>
            <p className="mb-8 text-sm text-fd-muted-foreground">
              MIT and Apache-2.0 licensed. No proprietary code, no upgrade tiers, no restrictions.
            </p>
            <div className="flex flex-wrap justify-center gap-4">
              <Link
                href="/docs/getting-started"
                className="inline-flex items-center gap-2 rounded-lg bg-fd-primary px-6 py-3 font-medium text-fd-primary-foreground transition-colors hover:bg-fd-primary/90"
              >
                <Zap className="h-4 w-4" />
                Get Started
              </Link>
              <Link
                href="/docs"
                className="inline-flex items-center gap-2 rounded-lg border border-fd-border bg-fd-background px-6 py-3 font-medium transition-colors hover:bg-fd-muted"
              >
                Documentation
              </Link>
              <a
                href="https://github.com/ScriptSmith/hadrian"
                className="inline-flex items-center gap-2 rounded-lg border border-fd-border bg-fd-background px-6 py-3 font-medium transition-colors hover:bg-fd-muted"
                target="_blank"
                rel="noopener noreferrer"
              >
                <GitHubIcon className="h-4 w-4" />
                GitHub
              </a>
            </div>
          </div>

          {/* Quick Start Selector */}
          <div className="mx-auto mt-12 max-w-6xl">
            <h2 className="mb-4 text-lg font-semibold">Get Started</h2>
            <QuickStartSelector />
          </div>
        </div>
      </section>

      {/* See it in Action */}
      <section className="border-b bg-fd-muted/30 py-16 md:py-24">
        <h2 className="mb-8 text-center text-3xl font-bold">See it in Action</h2>
        <DemoGallery />
      </section>

      {/* Providers */}
      <section className="border-b py-16 md:py-24">
        <div className="mx-auto max-w-6xl px-4">
          <h2 className="mb-4 text-center text-3xl font-bold">Providers</h2>
          <p className="mx-auto mb-12 max-w-2xl text-center text-fd-muted-foreground">
            Route to any provider through a unified API. Automatic failover, health checks, and
            circuit breakers included.
          </p>

          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {providers.map((p) => (
              <div key={p.name} className="rounded-lg border border-fd-border bg-fd-card px-4 py-3">
                <p className="font-medium">{p.name}</p>
                <p className="mt-0.5 text-sm text-fd-muted-foreground">{p.description}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Everything Included */}
      <section className="border-b bg-fd-muted/30 py-16 md:py-24">
        <div className="mx-auto max-w-6xl px-4">
          <h2 className="mb-4 text-center text-3xl font-bold">Everything Included</h2>
          <p className="mx-auto mb-12 max-w-2xl text-center text-fd-muted-foreground">
            Every feature is included in the open-source release. No asterisks, no upgrade walls.
          </p>
          <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
            {featureCategories.map((cat) => (
              <div key={cat.title} className="rounded-lg border border-fd-border bg-fd-card p-6">
                <cat.icon className="mb-3 h-6 w-6 text-fd-primary" />
                <h3 className="mb-3 text-lg font-semibold">{cat.title}</h3>
                <ul className="space-y-1.5 text-sm text-fd-muted-foreground">
                  {cat.items.map((item) => (
                    <li key={item} className="flex items-start gap-2">
                      <span className="mt-2 h-1 w-1 shrink-0 rounded-full bg-fd-muted-foreground" />
                      {item}
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* CTA */}
      <section className="py-16 md:py-24">
        <div className="mx-auto max-w-6xl px-4 text-center">
          <h2 className="mb-4 text-3xl font-bold">Ready to Get Started?</h2>
          <p className="mx-auto mb-8 max-w-xl text-fd-muted-foreground">
            Deploy in minutes with a single binary. No external dependencies for basic use.
          </p>
          <div className="flex flex-wrap justify-center gap-4">
            <Link
              href="/docs/getting-started"
              className="inline-flex items-center gap-2 rounded-lg bg-fd-primary px-6 py-3 font-medium text-fd-primary-foreground transition-colors hover:bg-fd-primary/90"
            >
              Quick Start Guide
            </Link>
            <Link
              href="/docs/deployment"
              className="inline-flex items-center gap-2 rounded-lg border border-fd-border bg-fd-background px-6 py-3 font-medium transition-colors hover:bg-fd-muted"
            >
              Deployment Guide
            </Link>
            <a
              href="https://github.com/ScriptSmith/hadrian"
              className="inline-flex items-center gap-2 rounded-lg border border-fd-border bg-fd-background px-6 py-3 font-medium transition-colors hover:bg-fd-muted"
              target="_blank"
              rel="noopener noreferrer"
            >
              <GitHubIcon className="h-4 w-4" />
              GitHub
            </a>
          </div>
        </div>
      </section>

      {/* Footer */}
      <footer className="border-t py-8">
        <div className="mx-auto max-w-6xl px-4">
          <div className="flex flex-col items-center justify-between gap-4 text-sm text-fd-muted-foreground md:flex-row">
            <p>Open Source (MIT, Apache-2.0). All enterprise features included.</p>
            <div className="flex gap-6">
              <Link href="/docs" className="hover:text-fd-foreground">
                Documentation
              </Link>
              <a
                href="https://github.com/ScriptSmith/hadrian"
                className="hover:text-fd-foreground"
                target="_blank"
                rel="noopener noreferrer"
              >
                GitHub
              </a>
              <a
                href="https://github.com/ScriptSmith/hadrian/issues"
                className="hover:text-fd-foreground"
                target="_blank"
                rel="noopener noreferrer"
              >
                Issues
              </a>
            </div>
          </div>
        </div>
      </footer>
    </div>
  );
}
