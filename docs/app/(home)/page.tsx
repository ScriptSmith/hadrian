"use client";

import { useState, useEffect, useRef } from "react";
import Link from "next/link";
import {
  Server,
  Shield,
  Users,
  Zap,
  Eye,
  Code,
  Brain,
  Copy,
  Check,
  Download,
  X,
} from "lucide-react";
import { StoryEmbed } from "@/components/story-embed";

function GitHubIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden="true">
      <path d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0 1 12 6.844a9.59 9.59 0 0 1 2.504.337c1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.02 10.02 0 0 0 22 12.017C22 6.484 17.522 2 12 2Z" />
    </svg>
  );
}

// --- Quick Start Selector ---

type Method = "binary" | "docker" | "cargo";
type OS = "linux-x86_64" | "linux-arm64" | "macos-arm64" | "windows";
type Profile = "full" | "standard" | "minimal" | "tiny";
type Libc = "gnu" | "musl";

const osLabels: Record<OS, string> = {
  "linux-x86_64": "Linux x86_64",
  "linux-arm64": "Linux ARM64",
  "macos-arm64": "macOS ARM64",
  windows: "Windows",
};

const libcLabels: Record<Libc, string> = {
  gnu: "glibc",
  musl: "musl",
};

function getTarget(os: OS, libc: Libc): string {
  switch (os) {
    case "linux-x86_64":
      return libc === "musl" ? "x86_64-unknown-linux-musl" : "x86_64-unknown-linux-gnu";
    case "linux-arm64":
      return "aarch64-unknown-linux-gnu";
    case "macos-arm64":
      return "aarch64-apple-darwin";
    case "windows":
      return "x86_64-pc-windows-msvc";
  }
}

const profileSummaries: Record<Profile, string> = {
  full: "Everything",
  standard: "Production deployment",
  minimal: "Development and embedded use",
  tiny: "Stateless proxy",
};

const featureMatrix: { name: string; profiles: Profile[] }[] = [
  { name: "OpenAI", profiles: ["tiny", "minimal", "standard", "full"] },
  { name: "Anthropic", profiles: ["minimal", "standard", "full"] },
  { name: "AWS Bedrock", profiles: ["minimal", "standard", "full"] },
  { name: "Google Vertex AI", profiles: ["minimal", "standard", "full"] },
  { name: "Azure OpenAI", profiles: ["minimal", "standard", "full"] },
  { name: "SQLite", profiles: ["minimal", "standard", "full"] },
  { name: "Embedded UI", profiles: ["minimal", "standard", "full"] },
  { name: "Setup wizard", profiles: ["minimal", "standard", "full"] },
  { name: "PostgreSQL", profiles: ["standard", "full"] },
  { name: "Redis caching", profiles: ["standard", "full"] },
  { name: "SSO (OIDC / OAuth)", profiles: ["standard", "full"] },
  { name: "CEL RBAC", profiles: ["standard", "full"] },
  { name: "S3 storage", profiles: ["standard", "full"] },
  { name: "Secrets managers", profiles: ["standard", "full"] },
  { name: "OTLP & Prometheus", profiles: ["standard", "full"] },
  { name: "OpenAPI docs", profiles: ["standard", "full"] },
  { name: "Doc extraction", profiles: ["standard", "full"] },
  { name: "SAML SSO", profiles: ["full"] },
  { name: "Kreuzberg OCR", profiles: ["full"] },
  { name: "ClamAV scanning", profiles: ["full"] },
];

function getInstallCommand(method: Method, os: OS, profile: Profile, libc: Libc): string {
  if (method === "docker") {
    return [
      "docker run \\",
      "  -p 8080:8080 \\",
      "  -e OPENROUTER_API_KEY=sk-... \\",
      "  ghcr.io/scriptsmith/hadrian",
    ].join("\n");
  }
  if (method === "cargo") {
    return "cargo install hadrian\nhadrian";
  }
  const ext = os === "windows" ? "zip" : "tar.gz";
  const target = getTarget(os, libc);
  const filename = `hadrian-${target}-${profile}.${ext}`;
  const url = `https://github.com/ScriptSmith/hadrian/releases/latest/download/${filename}`;
  if (os === "windows") {
    return [`curl -LO \\`, `  ${url}`, `tar -xf ${filename}`, `.\\hadrian.exe`].join("\n");
  }
  return [`curl -L \\`, `  ${url} \\`, `  | tar xz`, `./hadrian`].join("\n");
}

function getDownloadUrl(os: OS, profile: Profile, libc: Libc): string {
  const ext = os === "windows" ? "zip" : "tar.gz";
  const target = getTarget(os, libc);
  return `https://github.com/ScriptSmith/hadrian/releases/latest/download/hadrian-${target}-${profile}.${ext}`;
}

function ToggleGroup<T extends string>({
  options,
  value,
  onChange,
  labels,
  disabled,
}: {
  options: T[];
  value: T;
  onChange: (v: T) => void;
  labels?: Record<T, string>;
  disabled?: Set<T>;
}) {
  return (
    <div className="flex flex-wrap gap-1.5">
      {options.map((opt) => {
        const isDisabled = disabled?.has(opt);
        return (
          <button
            key={opt}
            onClick={() => onChange(opt)}
            disabled={isDisabled}
            className={`rounded-md px-3 py-1.5 text-sm font-medium transition-colors ${
              isDisabled
                ? "cursor-not-allowed bg-fd-muted text-fd-muted-foreground/40"
                : value === opt
                  ? "bg-fd-primary text-fd-primary-foreground"
                  : "bg-fd-muted text-fd-muted-foreground hover:bg-fd-muted/80 hover:text-fd-foreground"
            }`}
          >
            {labels ? labels[opt] : opt}
          </button>
        );
      })}
    </div>
  );
}

function getDisabledProfiles(os: OS, libc: Libc): Set<Profile> | undefined {
  if (os === "windows") return new Set(["full", "standard"]);
  if (os === "linux-arm64") return new Set(["full"]);
  if (os.startsWith("linux-") && libc === "musl") return new Set(["full"]);
  return undefined;
}

function QuickStartSelector() {
  const [method, setMethod] = useState<Method>("binary");
  const [os, setOs] = useState<OS>("linux-x86_64");
  const [profile, setProfile] = useState<Profile>("full");
  const [libc, setLibc] = useState<Libc>("gnu");
  const [copied, setCopied] = useState(false);

  const isLinux = os === "linux-x86_64" || os === "linux-arm64";
  const disabledProfiles = getDisabledProfiles(os, libc);
  const disabledLibcs = os === "linux-arm64" ? new Set<Libc>(["musl"]) : undefined;

  const handleOsChange = (newOs: OS) => {
    setOs(newOs);
    // Reset libc to gnu for non-Linux or ARM64 (no musl builds)
    let newLibc = libc;
    if (!newOs.startsWith("linux-") || newOs === "linux-arm64") {
      newLibc = "gnu";
      setLibc("gnu");
    }
    // Adjust profile if it becomes unavailable
    const disabled = getDisabledProfiles(newOs, newLibc);
    if (disabled?.has(profile)) {
      setProfile(disabled.has("standard") ? "minimal" : "standard");
    }
  };

  const handleLibcChange = (newLibc: Libc) => {
    setLibc(newLibc);
    if (newLibc === "musl" && profile === "full") {
      setProfile("standard");
    }
  };

  const command = getInstallCommand(method, os, profile, libc);
  const downloadUrl = method === "binary" ? getDownloadUrl(os, profile, libc) : null;

  const handleCopy = async () => {
    await navigator.clipboard.writeText(command);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="overflow-hidden rounded-lg border border-fd-border bg-fd-card">
      <div className="space-y-3 border-b border-fd-border bg-fd-muted/50 p-4">
        <div className="flex flex-wrap items-center gap-3">
          <span className="w-16 shrink-0 text-sm font-medium text-fd-muted-foreground">Method</span>
          <ToggleGroup
            options={["binary", "docker", "cargo"] as Method[]}
            value={method}
            onChange={setMethod}
            labels={{ binary: "Binary", docker: "Docker", cargo: "Cargo" }}
          />
        </div>
        {method === "binary" && (
          <>
            <div className="flex flex-wrap items-center gap-3">
              <span className="w-16 shrink-0 text-sm font-medium text-fd-muted-foreground">OS</span>
              <ToggleGroup
                options={["linux-x86_64", "linux-arm64", "macos-arm64", "windows"] as OS[]}
                value={os}
                onChange={handleOsChange}
                labels={osLabels}
              />
            </div>
            {isLinux && (
              <div className="flex flex-wrap items-center gap-3">
                <span className="w-16 shrink-0 text-sm font-medium text-fd-muted-foreground">
                  Libc
                </span>
                <ToggleGroup
                  options={["gnu", "musl"] as Libc[]}
                  value={libc}
                  onChange={handleLibcChange}
                  labels={libcLabels}
                  disabled={disabledLibcs}
                />
              </div>
            )}
            <div className="flex flex-wrap items-center gap-3">
              <span className="w-16 shrink-0 text-sm font-medium text-fd-muted-foreground">
                Features
              </span>
              <ToggleGroup
                options={["full", "standard", "minimal", "tiny"] as Profile[]}
                value={profile}
                onChange={setProfile}
                disabled={disabledProfiles}
              />
            </div>
          </>
        )}
      </div>

      {/* Feature matrix — shown for binary installs */}
      {method === "binary" && (
        <div className="border-b border-fd-border bg-fd-muted/20 px-4 py-3">
          <p className="mb-2 text-sm font-medium text-fd-foreground">{profileSummaries[profile]}</p>
          <div className="grid grid-cols-2 gap-x-6 gap-y-1 sm:grid-cols-3">
            {featureMatrix.map((f) => {
              const included = f.profiles.includes(profile);
              return (
                <div key={f.name} className="flex items-center gap-2 text-sm">
                  {included ? (
                    <Check className="h-3.5 w-3.5 shrink-0 text-green-500" />
                  ) : (
                    <X className="h-3.5 w-3.5 shrink-0 text-fd-muted-foreground/40" />
                  )}
                  <span
                    className={
                      included ? "text-fd-foreground" : "text-fd-muted-foreground/50 line-through"
                    }
                  >
                    {f.name}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      )}

      <div className="relative">
        <pre className="overflow-x-auto whitespace-pre-wrap break-all p-4 pr-12 text-sm">
          <code className="text-fd-foreground">{command}</code>
        </pre>
        <button
          onClick={handleCopy}
          className="absolute right-3 top-3 rounded-md p-1.5 text-fd-muted-foreground transition-colors hover:bg-fd-muted hover:text-fd-foreground"
          aria-label="Copy command"
        >
          {copied ? <Check className="h-4 w-4 text-green-500" /> : <Copy className="h-4 w-4" />}
        </button>
      </div>

      {downloadUrl && (
        <div className="border-t border-fd-border bg-fd-muted/30 px-4 py-3">
          <a
            href={downloadUrl}
            className="inline-flex items-center gap-2 rounded-lg bg-fd-primary px-4 py-2 text-sm font-medium text-fd-primary-foreground transition-colors hover:bg-fd-primary/90"
          >
            <Download className="h-4 w-4" />
            Download binary
          </a>
          <p className="mt-2 break-all text-xs text-fd-muted-foreground">
            <a href={downloadUrl} className="underline">
              {downloadUrl}
            </a>
          </p>
        </div>
      )}
    </div>
  );
}

// --- See it in Action (Gallery) ---

const demos = [
  {
    id: "studio",
    title: "Studio",
    description: "Generate images across providers simultaneously with cost tracking.",
    storyId: "pages-studiopage--images",
  },
  {
    id: "chat",
    title: "Multi-Model Chat",
    description:
      "Compare responses from multiple models side-by-side with advanced multi-model modes.",
    storyId: "chat-chatview--multi-model-conversation",
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
    chatRef.current?.scrollIntoView({ inline: "center", block: "nearest" });
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
