"use client";

import { useState } from "react";
import { Check, Copy, Download, X } from "lucide-react";

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
    return ["docker run \\", "  -p 8080:8080 \\", "  ghcr.io/scriptsmith/hadrian"].join("\n");
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

export function QuickStartSelector() {
  const [method, setMethod] = useState<Method>("binary");
  const [os, setOs] = useState<OS>("linux-x86_64");
  const [profile, setProfile] = useState<Profile>("standard");
  const [libc, setLibc] = useState<Libc>("musl");
  const [copied, setCopied] = useState(false);

  const isLinux = os === "linux-x86_64" || os === "linux-arm64";
  const disabledProfiles = getDisabledProfiles(os, libc);
  const disabledLibcs = os === "linux-arm64" ? new Set<Libc>(["musl"]) : undefined;

  const handleOsChange = (newOs: OS) => {
    setOs(newOs);
    let newLibc = libc;
    if (!newOs.startsWith("linux-") || newOs === "linux-arm64") {
      newLibc = "gnu";
      setLibc("gnu");
    }
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
    if (navigator.clipboard) {
      await navigator.clipboard.writeText(command);
    } else {
      const textarea = document.createElement("textarea");
      textarea.value = command;
      textarea.style.position = "fixed";
      textarea.style.opacity = "0";
      document.body.appendChild(textarea);
      textarea.select();
      document.execCommand("copy");
      document.body.removeChild(textarea);
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="not-prose overflow-hidden rounded-lg border border-fd-border bg-fd-card">
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
