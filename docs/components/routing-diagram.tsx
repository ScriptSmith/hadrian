"use client";

import Link from "next/link";
import { Plug, Server, User } from "lucide-react";
import { Anthropic, AzureAI, Bedrock, Gemini, Ollama, OpenAI, OpenRouter } from "@lobehub/icons";

const basePath = process.env.DOCS_BASE_PATH || "";
const PROVIDERS_DOCS = "/docs/configuration/providers";

// --- Geometry (viewBox units) ---
const VB_W = 960;
const VB_H = 560;
const UX = 96; // user node center x
const UY = 280; // shared vertical center
const GX = 470; // gateway center x
const GY = 280;
const PX = 770; // provider chip center x
const PROVIDER_HALF = 22; // provider chip radius
const ROW_GAP = 56; // vertical spacing between provider rows

// Fraction of each packet's cycle spent travelling (the rest is an idle gap, so
// requests fire at scattered, irregular times rather than as a steady stream).
const TRAVEL = 0.45;

// Providers, sorted alphabetically. Each links to its docs section and carries
// its own packet timing so the routing animation looks random, not a sweep.
const providers: {
  name: string;
  node: React.ReactNode;
  href: string;
  dur: number;
  begin: number;
}[] = [
  {
    name: "Amazon Bedrock",
    node: <Bedrock.Color size={24} />,
    href: `${PROVIDERS_DOCS}#aws-bedrock`,
    dur: 2.6,
    begin: 0,
  },
  {
    name: "Anthropic",
    node: <Anthropic size={22} style={{ color: "#D97757" }} />,
    href: `${PROVIDERS_DOCS}#anthropic`,
    dur: 3.3,
    begin: 1.5,
  },
  {
    name: "Azure OpenAI",
    node: <AzureAI.Color size={24} />,
    href: `${PROVIDERS_DOCS}#azure-openai`,
    dur: 2.1,
    begin: 0.8,
  },
  {
    name: "Google Gemini",
    node: <Gemini.Color size={24} />,
    href: `${PROVIDERS_DOCS}#google-vertex-ai`,
    dur: 2.9,
    begin: 2.2,
  },
  {
    name: "Ollama",
    node: <Ollama size={22} className="text-fd-foreground" />,
    href: `${PROVIDERS_DOCS}#openai-compatible-providers`,
    dur: 3.5,
    begin: 0.5,
  },
  {
    name: "On-prem",
    node: <Server size={20} strokeWidth={1.75} className="text-fd-muted-foreground" />,
    href: `${PROVIDERS_DOCS}#openai-compatible-providers`,
    dur: 2.3,
    begin: 1.9,
  },
  {
    name: "OpenAI",
    node: <OpenAI size={22} className="text-fd-foreground" />,
    href: `${PROVIDERS_DOCS}#openai`,
    dur: 2.8,
    begin: 0.3,
  },
  {
    name: "OpenAI-compatible",
    node: <Plug size={20} strokeWidth={1.75} className="text-fd-muted-foreground" />,
    href: `${PROVIDERS_DOCS}#openai-compatible-providers`,
    dur: 3.1,
    begin: 2.6,
  },
  {
    name: "OpenRouter",
    node: <OpenRouter size={22} style={{ color: "#6566F1" }} />,
    href: `${PROVIDERS_DOCS}#openai-compatible-providers`,
    dur: 2.4,
    begin: 1.2,
  },
];

// Vertical positions for the provider column, centered on UY.
const providerYs = providers.map((_, i) => UY + (i - (providers.length - 1) / 2) * ROW_GAP);

// Path from the gateway out to a given provider row.
const providerPath = (y: number) =>
  `M512,${GY} C 624,${GY} 644,${y} ${PX - PROVIDER_HALF - 2},${y}`;

// Path from the user into the gateway.
const userPath = `M${UX + 36},${UY} L${GX - 40},${GY}`;

// A discrete request packet: travels gateway -> provider, then idles (a gap)
// until the next cycle. Scattered durations make the overall pattern feel random.
function PacketDot({ path, dur, begin }: { path: string; dur: number; begin: number }) {
  const d = `${dur}s`;
  const b = `${begin}s`;
  return (
    <circle
      r="4.5"
      className="fill-fd-primary motion-reduce:hidden"
      opacity={0}
      style={{ filter: "url(#hadrian-dot-glow)" }}
    >
      <animateMotion
        path={path}
        dur={d}
        begin={b}
        repeatCount="indefinite"
        calcMode="linear"
        keyPoints="0;1;1"
        keyTimes={`0;${TRAVEL};1`}
      />
      <animate
        attributeName="opacity"
        values="0;1;1;0;0"
        keyTimes={`0;0.04;${TRAVEL - 0.03};${TRAVEL + 0.01};1`}
        dur={d}
        begin={b}
        repeatCount="indefinite"
      />
    </circle>
  );
}

// A soft halo that flashes as a packet reaches a provider chip.
function NodeGlow({
  x,
  y,
  size,
  dur,
  begin,
}: {
  x: number;
  y: number;
  size: number;
  dur: number;
  begin: number;
}) {
  return (
    <rect
      x={x - size / 2}
      y={y - size / 2}
      width={size}
      height={size}
      rx={size / 3}
      aria-hidden="true"
      className="fill-fd-primary motion-reduce:hidden"
      opacity={0}
      style={{ filter: "url(#hadrian-node-glow)" }}
    >
      <animate
        attributeName="opacity"
        values="0;0;0.65;0;0"
        keyTimes={`0;${TRAVEL - 0.05};${TRAVEL + 0.02};${TRAVEL + 0.17};1`}
        dur={`${dur}s`}
        begin={`${begin}s`}
        repeatCount="indefinite"
      />
    </rect>
  );
}

// Steady stream of inbound requests from the user into the gateway.
function StreamDot({ dur, begin }: { dur: number; begin: number }) {
  const d = `${dur}s`;
  const b = `${begin}s`;
  return (
    <circle
      r="4.5"
      className="fill-fd-primary motion-reduce:hidden"
      opacity={0}
      style={{ filter: "url(#hadrian-dot-glow)" }}
    >
      <animateMotion path={userPath} dur={d} begin={b} repeatCount="indefinite" />
      <animate
        attributeName="opacity"
        values="0;1;1;0"
        keyTimes="0;0.12;0.85;1"
        dur={d}
        begin={b}
        repeatCount="indefinite"
      />
    </circle>
  );
}

export function RoutingDiagram() {
  return (
    <div className="overflow-x-auto">
      <svg
        viewBox={`0 0 ${VB_W} ${VB_H}`}
        aria-label="Hadrian Gateway routes client requests to any provider"
        className="mx-auto h-auto w-full min-w-[720px] max-w-3xl"
      >
        <defs>
          <filter id="hadrian-dot-glow" x="-200%" y="-200%" width="500%" height="500%">
            <feGaussianBlur stdDeviation="2.5" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
          <filter id="hadrian-node-glow" x="-100%" y="-100%" width="300%" height="300%">
            <feGaussianBlur stdDeviation="7" />
          </filter>
        </defs>

        {/* Connection wires */}
        <g fill="none" aria-hidden="true" className="stroke-fd-border" strokeWidth={1.5}>
          <path d={userPath} />
          {providerYs.map((y) => (
            <path key={y} d={providerPath(y)} />
          ))}
        </g>

        {/* Routing flow */}
        <g aria-hidden="true">
          <StreamDot dur={1.4} begin={0} />
          <StreamDot dur={1.7} begin={0.9} />
          {providers.map((p, i) => (
            <PacketDot
              key={p.name}
              path={providerPath(providerYs[i])}
              dur={p.dur}
              begin={p.begin}
            />
          ))}
        </g>

        {/* User node */}
        <foreignObject x={UX - 36} y={UY - 36} width={72} height={72} aria-hidden="true">
          <div className="flex h-full w-full items-center justify-center rounded-2xl border border-fd-border bg-fd-card shadow-sm">
            <User className="h-8 w-8 text-fd-muted-foreground" strokeWidth={1.5} />
          </div>
        </foreignObject>
        <text
          x={UX}
          y={UY + 56}
          textAnchor="middle"
          className="fill-fd-foreground"
          fontSize={17}
          fontWeight={600}
        >
          Your users
        </text>

        {/* Gateway node */}
        <image href={`${basePath}/icon.svg`} x={GX - 42} y={GY - 42} width={84} height={84} />
        <text
          x={GX}
          y={GY + 72}
          textAnchor="middle"
          className="fill-fd-foreground"
          fontSize={19}
          fontWeight={700}
        >
          Hadrian Gateway
        </text>

        {/* Provider nodes (each links to its docs section) */}
        {providers.map((p, i) => {
          const y = providerYs[i];
          return (
            <g key={p.name}>
              <NodeGlow x={PX} y={y} size={60} dur={p.dur} begin={p.begin} />
              <foreignObject
                x={PX - PROVIDER_HALF}
                y={y - PROVIDER_HALF}
                width={VB_W - (PX - PROVIDER_HALF)}
                height={PROVIDER_HALF * 2}
              >
                <Link
                  href={p.href}
                  aria-label={`${p.name} provider documentation`}
                  className="group flex h-full items-center gap-3 no-underline"
                >
                  <span className="flex aspect-square h-full flex-none items-center justify-center rounded-xl border border-fd-border bg-fd-card shadow-sm transition-colors group-hover:border-fd-primary/60">
                    {p.node}
                  </span>
                  <span
                    className="font-medium text-fd-muted-foreground transition-colors group-hover:text-fd-foreground"
                    style={{ fontSize: 15 }}
                  >
                    {p.name}
                  </span>
                </Link>
              </foreignObject>
            </g>
          );
        })}
      </svg>
    </div>
  );
}
