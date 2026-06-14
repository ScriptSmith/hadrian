"use client";

import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
import Link from "next/link";
import {
  Database,
  FileSearch,
  Fingerprint,
  Gauge,
  Globe,
  Plug,
  ScrollText,
  Server,
  ShieldAlert,
  ShieldCheck,
  Terminal,
  User,
  Wallet,
} from "lucide-react";
import { Anthropic, AzureAI, Bedrock, Gemini, Ollama, OpenAI, OpenRouter } from "@lobehub/icons";

const basePath = process.env.DOCS_BASE_PATH || "";
const PROVIDERS_DOCS = "/docs/configuration/providers";

// --- Geometry (viewBox units) ---
const VB_W = 960;
const VB_H = 560;
const UX = 110; // user node center x
const UY = 280; // shared vertical center
const GX = 470; // gateway center x
const GY = 280;
const PX = 770; // provider chip center x
const GW_HALF = 42; // gateway icon half-size
const PROVIDER_HALF = 22; // provider chip radius
const ROW_GAP = 56; // vertical spacing between provider rows

const GW_RIGHT = GX + GW_HALF;
const GW_LEFT = GX - GW_HALF;
const GW_BOTTOM = GY + GW_HALF;

const DOT_FILTER = { filter: "url(#hadrian-dot-glow)" } as const;

// Vertical positions for n provider rows, centered on UY.
const providerYs = (n: number) =>
  Array.from({ length: n }, (_, i) => UY + (i - (n - 1) / 2) * ROW_GAP);

// Gateway out to a provider row.
const providerPath = (y: number) =>
  `M${GW_RIGHT},${GY} C ${GW_RIGHT + 112},${GY} ${GW_RIGHT + 132},${y} ${PX - PROVIDER_HALF - 2},${y}`;

// Full lane: user straight through the gateway, then on to one provider. A single
// dot on this path is one request reaching exactly one provider (no split).
const A_IN = { x: UX + 36, y: UY };
const fullPath = (y: number) =>
  `M${A_IN.x},${A_IN.y} L${GW_RIGHT},${GY} C ${GW_RIGHT + 112},${GY} ${GW_RIGHT + 132},${y} ${PX - PROVIDER_HALF - 2},${y}`;

// Inbound only: user into the gateway.
const userPath = `M${A_IN.x},${A_IN.y} L${GW_LEFT - 4},${GY}`;

// Gateway down to a node directly beneath it (logs, cache, meters).
const SINK_TOP = 384;
const sinkPath = `M${GX},${GW_BOTTOM} L${GX},${SINK_TOP}`;

// =====================================================================
// Constant-speed timing
//
// Every request dot moves at the same pixel speed in every diagram. A dot's
// travel time is therefore its path length / SPEED, computed analytically so we
// never depend on the DOM. Lanes share a period so the launch cadence and the
// number of requests in flight stay consistent across scenes.
// =====================================================================

const SPEED = 170; // viewBox units per second — the one speed used everywhere
const IN_FLIGHT = 3; // target number of requests visible at once, per scene

// Length of an "M L … C …" path (straight segments exact, curves sampled).
function pathLength(d: string): number {
  const t = d.match(/[MLC]|-?\d+(?:\.\d+)?/g);
  if (!t) return 0;
  let i = 0;
  const num = () => parseFloat(t[i++]);
  let cx = 0,
    cy = 0,
    len = 0;
  while (i < t.length) {
    const cmd = t[i++];
    if (cmd === "M") {
      cx = num();
      cy = num();
    } else if (cmd === "L") {
      const x = num(),
        y = num();
      // Math.sqrt is IEEE-754 correctly-rounded (deterministic across V8s);
      // Math.hypot is not, which would desync SSR vs client durations.
      len += Math.sqrt((x - cx) * (x - cx) + (y - cy) * (y - cy));
      cx = x;
      cy = y;
    } else if (cmd === "C") {
      const x1 = num(),
        y1 = num(),
        x2 = num(),
        y2 = num(),
        x = num(),
        y = num();
      let px = cx,
        py = cy;
      for (let s = 1; s <= 24; s++) {
        const u = s / 24,
          m = 1 - u;
        const bx = m * m * m * cx + 3 * m * m * u * x1 + 3 * m * u * u * x2 + u * u * u * x;
        const by = m * m * m * cy + 3 * m * m * u * y1 + 3 * m * u * u * y2 + u * u * u * y;
        len += Math.sqrt((bx - px) * (bx - px) + (by - py) * (by - py));
        px = bx;
        py = by;
      }
      cx = x;
      cy = y;
    }
  }
  return len;
}

const travelTime = (d: string) => pathLength(d) / SPEED;
// Fraction of a full lane reached at the gateway centre (where colour changes).
const gateFrac = (d: string) => (GX - A_IN.x) / pathLength(d);

// Shared period for a set of lanes: keeps ~IN_FLIGHT dots visible and a steady
// launch cadence regardless of how many lanes the scene has.
function lanePeriod(times: number[]): number {
  const n = times.length;
  const maxT = Math.max(...times);
  return Math.max((n / IN_FLIGHT) * maxT, 1.05 * maxT);
}
// Low-discrepancy stagger so launches never look like a top-to-bottom sweep.
const laneBegin = (i: number, period: number) => Number((((i * 0.618033) % 1) * period).toFixed(3));

// =====================================================================
// Primitives
// =====================================================================

// A constant-speed request dot: travels `path` once per `dur`, then idles. The
// caller passes `dur`; the travelling fraction is derived from the path length
// so the on-screen speed is identical to every other dot.
function Flow({
  path,
  dur,
  begin,
  className = "fill-fd-primary",
  r = 4.5,
}: {
  path: string;
  dur: number;
  begin: number;
  className?: string;
  r?: number;
}) {
  const travel = Math.min(0.985, travelTime(path) / dur);
  const d = `${dur}s`;
  const b = `${begin}s`;
  return (
    <circle r={r} className={`${className} motion-reduce:hidden`} opacity={0} style={DOT_FILTER}>
      <animateMotion
        path={path}
        dur={d}
        begin={b}
        repeatCount="indefinite"
        calcMode="linear"
        keyPoints="0;1;1"
        keyTimes={`0;${travel};1`}
      />
      <animate
        attributeName="opacity"
        values="0;1;1;0;0"
        keyTimes={`0;0.03;${(travel - 0.03).toFixed(3)};${(travel + 0.01).toFixed(3)};1`}
        dur={d}
        begin={b}
        repeatCount="indefinite"
      />
    </circle>
  );
}

// A constant-speed dot that changes colour as it passes the gateway centre.
function TwoColorFlow({
  path,
  dur,
  begin,
  inClass = "fill-fd-primary",
  outClass,
}: {
  path: string;
  dur: number;
  begin: number;
  inClass?: string;
  outClass: string;
}) {
  const travel = Math.min(0.985, travelTime(path) / dur);
  const tg = Number((gateFrac(path) * travel).toFixed(3));
  const d = `${dur}s`;
  const b = `${begin}s`;
  const motion = (
    <animateMotion
      path={path}
      dur={d}
      begin={b}
      repeatCount="indefinite"
      calcMode="linear"
      keyPoints="0;1;1"
      keyTimes={`0;${travel};1`}
    />
  );
  return (
    <>
      <circle r="4.5" className={`${inClass} motion-reduce:hidden`} opacity={0} style={DOT_FILTER}>
        {motion}
        <animate
          attributeName="opacity"
          values="0;1;1;0;0"
          keyTimes={`0;0.03;${(tg - 0.01).toFixed(3)};${tg};1`}
          dur={d}
          begin={b}
          repeatCount="indefinite"
        />
      </circle>
      <circle r="4.5" className={`${outClass} motion-reduce:hidden`} opacity={0} style={DOT_FILTER}>
        {motion}
        <animate
          attributeName="opacity"
          values="0;0;1;1;0;0"
          keyTimes={`0;${tg};${(tg + 0.01).toFixed(3)};${(travel - 0.01).toFixed(3)};${travel};1`}
          dur={d}
          begin={b}
          repeatCount="indefinite"
        />
      </circle>
    </>
  );
}

// A request that reaches `peak` along `path`, then reverses — denied, blocked,
// or rate-limited. Both legs move at SPEED (span derived from the path length).
function ReturnDot({
  path,
  peak,
  dur,
  begin,
  at,
  inClass = "fill-fd-primary",
  outClass,
}: {
  path: string;
  peak: number;
  dur: number;
  begin: number;
  at: number;
  inClass?: string;
  outClass: string;
}) {
  // Each leg runs at SPEED; clamp only to keep the bounce inside one cycle.
  const span = Math.min((peak * pathLength(path)) / SPEED / dur, at - 0.001, 0.999 - at);
  const t0 = Math.max(0.0001, at - span);
  const keyPoints = `0;0;${peak};0;0`;
  const keyTimes = `0;${t0.toFixed(3)};${at};${(at + span).toFixed(3)};1`;
  const motion = (
    <animateMotion
      path={path}
      dur={`${dur}s`}
      begin={`${begin}s`}
      repeatCount="indefinite"
      calcMode="linear"
      keyPoints={keyPoints}
      keyTimes={keyTimes}
    />
  );
  return (
    <>
      <circle r="4.5" className={`${inClass} motion-reduce:hidden`} opacity={0} style={DOT_FILTER}>
        {motion}
        <animate
          attributeName="opacity"
          values="0;0;1;1;0;0"
          keyTimes={`0;${t0.toFixed(3)};${(t0 + 0.02).toFixed(3)};${(at - 0.01).toFixed(3)};${at};1`}
          dur={`${dur}s`}
          begin={`${begin}s`}
          repeatCount="indefinite"
        />
      </circle>
      <circle r="4.5" className={`${outClass} motion-reduce:hidden`} opacity={0} style={DOT_FILTER}>
        {motion}
        <animate
          attributeName="opacity"
          values="0;0;1;1;0;0"
          keyTimes={`0;${at};${(at + 0.02).toFixed(3)};${(at + span - 0.01).toFixed(3)};${(at + span).toFixed(3)};1`}
          dur={`${dur}s`}
          begin={`${begin}s`}
          repeatCount="indefinite"
        />
      </circle>
    </>
  );
}

// A halo that flashes as a dot reaches a node (`at` = arrival fraction of dur).
function NodeGlow({
  x,
  y,
  size,
  dur,
  begin,
  at,
}: {
  x: number;
  y: number;
  size: number;
  dur: number;
  begin: number;
  at: number;
}) {
  // A fixed ~0.55s pulse that peaks exactly as the dot arrives (`at`), so the
  // glow stays in step whatever the lane period is.
  const a1 = Math.max(0, at - 0.16 / dur).toFixed(4);
  const a2 = at.toFixed(4);
  const a3 = Math.min(1, at + 0.4 / dur).toFixed(4);
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
        values="0;0;0.7;0;0"
        keyTimes={`0;${a1};${a2};${a3};1`}
        dur={`${dur}s`}
        begin={`${begin}s`}
        repeatCount="indefinite"
      />
    </rect>
  );
}

// =====================================================================
// Node chips
// =====================================================================

function UserNode({ label = "Your users" }: { label?: string }) {
  return (
    <>
      <foreignObject x={UX - 34} y={UY - 34} width={68} height={68} aria-hidden="true">
        <div className="flex h-full w-full items-center justify-center rounded-2xl border border-fd-border bg-fd-card shadow-sm">
          <User className="h-7 w-7 text-fd-muted-foreground" strokeWidth={1.5} />
        </div>
      </foreignObject>
      <text
        x={UX}
        y={UY + 54}
        textAnchor="middle"
        className="fill-fd-foreground"
        fontSize={16}
        fontWeight={600}
      >
        {label}
      </text>
    </>
  );
}

function GatewayNode() {
  return (
    <>
      <image
        href={`${basePath}/icon.svg`}
        x={GX - GW_HALF}
        y={GY - GW_HALF}
        width={GW_HALF * 2}
        height={GW_HALF * 2}
      />
      <text
        x={GX}
        y={GW_BOTTOM + 30}
        textAnchor="middle"
        className="fill-fd-foreground"
        fontSize={18}
        fontWeight={700}
      >
        Hadrian Gateway
      </text>
    </>
  );
}

type Provider = {
  name: string;
  node: React.ReactNode;
  href: string;
};

function Chip({
  provider,
  y,
  region,
  regionColor,
}: {
  provider: Provider;
  y: number;
  region?: string;
  regionColor?: string;
}) {
  return (
    <foreignObject
      x={PX - PROVIDER_HALF}
      y={y - PROVIDER_HALF}
      width={VB_W - (PX - PROVIDER_HALF)}
      height={PROVIDER_HALF * 2}
    >
      <Link
        href={provider.href}
        aria-label={`${provider.name} provider documentation`}
        className="group flex h-full items-center gap-3 no-underline"
      >
        <span className="flex aspect-square h-full flex-none items-center justify-center rounded-xl border border-fd-border bg-fd-card shadow-sm transition-colors group-hover:border-fd-primary/60">
          {provider.node}
        </span>
        <span className="flex flex-col leading-tight">
          <span
            className="font-medium text-fd-muted-foreground transition-colors group-hover:text-fd-foreground"
            style={{ fontSize: 14 }}
          >
            {provider.name}
          </span>
          {region && (
            <span className="flex items-center gap-1 text-[11px] uppercase tracking-wide text-fd-muted-foreground/70">
              <span
                className="inline-block h-2 w-2 rounded-full"
                style={{ background: regionColor }}
              />
              {region}
            </span>
          )}
        </span>
      </Link>
    </foreignObject>
  );
}

function ProviderChips({ providers }: { providers: Provider[] }) {
  const ys = providerYs(providers.length);
  return (
    <>
      {providers.map((p, i) => (
        <Chip key={p.name} provider={p} y={ys[i]} />
      ))}
    </>
  );
}

function Wires({ providers }: { providers: Provider[] }) {
  const ys = providerYs(providers.length);
  return (
    <g fill="none" aria-hidden="true" className="stroke-fd-border" strokeWidth={1.5}>
      <path d={userPath} />
      {ys.map((y, i) => (
        <path key={i} d={providerPath(y)} />
      ))}
    </g>
  );
}

// One constant-speed dot per provider lane (+ arrival glow). Optionally turns a
// second colour at the gateway (auth/authz "allowed" requests).
function Lanes({
  providers,
  className = "fill-fd-primary",
  outClass,
  sizes,
}: {
  providers: Provider[];
  className?: string;
  outClass?: string;
  sizes?: number[];
}) {
  const ys = providerYs(providers.length);
  const paths = ys.map(fullPath);
  const times = paths.map(travelTime);
  const period = lanePeriod(times);
  return (
    <>
      {paths.map((d, i) => {
        const begin = laneBegin(i, period);
        const at = Math.min(0.985, times[i] / period);
        return (
          <g key={i}>
            <NodeGlow x={PX} y={ys[i]} size={56} dur={period} begin={begin} at={at} />
            {outClass ? (
              <TwoColorFlow
                path={d}
                dur={period}
                begin={begin}
                inClass={className}
                outClass={outClass}
              />
            ) : (
              <Flow
                path={d}
                dur={period}
                begin={begin}
                className={className}
                r={sizes ? sizes[i % sizes.length] : 4.5}
              />
            )}
          </g>
        );
      })}
    </>
  );
}

// The launch cadence (and log roll interval) for a set of providers.
const cadenceFor = (providers: Provider[]) => {
  const times = providerYs(providers.length).map((y) => travelTime(fullPath(y)));
  return lanePeriod(times) / providers.length;
};

function SatelliteNode({
  x,
  y,
  w,
  h,
  icon,
  title,
  subtitle,
}: {
  x: number;
  y: number;
  w: number;
  h: number;
  icon: React.ReactNode;
  title: string;
  subtitle?: string;
}) {
  return (
    <foreignObject x={x - w / 2} y={y - h / 2} width={w} height={h} aria-hidden="true">
      <div className="flex h-full w-full items-center gap-2.5 rounded-xl border border-fd-border bg-fd-card px-3 shadow-sm">
        <span className="flex-none text-fd-muted-foreground">{icon}</span>
        <span className="flex flex-col leading-tight">
          <span className="text-[13px] font-semibold text-fd-foreground">{title}</span>
          {subtitle && <span className="text-[11px] text-fd-muted-foreground">{subtitle}</span>}
        </span>
      </div>
    </foreignObject>
  );
}

// A live log pinned under the gateway. Rows step upward once per `step` seconds
// (the request cadence) so entries roll in time with the request dots.
function RollingLog({
  w,
  title,
  icon,
  href,
  rows,
  step,
}: {
  w: number;
  title: string;
  icon: React.ReactNode;
  href?: string;
  rows: React.ReactNode[];
  step: number;
}) {
  const rowH = 16;
  const visible = 3;
  const dur = rows.length * step;
  const h = 30 + visible * rowH + 8;
  const header = (
    <span className="flex items-center gap-1.5 text-[12px] font-semibold text-fd-foreground">
      <span className="text-fd-muted-foreground">{icon}</span>
      {title}
    </span>
  );
  return (
    <foreignObject
      x={GX - w / 2}
      y={SINK_TOP}
      width={w}
      height={h}
      aria-hidden={href ? undefined : "true"}
    >
      <div className="h-full w-full rounded-xl border border-fd-border bg-fd-card px-3 py-2 shadow-sm">
        <div className="mb-1">
          {href ? (
            <Link
              href={href}
              className="inline-flex no-underline transition-colors hover:text-fd-primary"
            >
              {header}
            </Link>
          ) : (
            header
          )}
        </div>
        <div style={{ height: visible * rowH, overflow: "hidden" }}>
          <div
            className="hadrian-noanim font-mono text-[10.5px]"
            style={{ animation: `hadrian-roll ${dur}s steps(${rows.length}) infinite` }}
          >
            {[...rows, ...rows].map((row, i) => (
              <div
                key={i}
                className="flex items-center gap-1.5 overflow-hidden whitespace-nowrap"
                style={{ height: rowH }}
              >
                {row}
              </div>
            ))}
          </div>
        </div>
      </div>
    </foreignObject>
  );
}

function Tag({ tone, children }: { tone: "allow" | "deny" | "redact"; children: React.ReactNode }) {
  const map = {
    allow: ["#16a34a", "rgba(22,163,74,0.12)"],
    deny: ["#dc2626", "rgba(220,38,38,0.12)"],
    redact: ["#d97706", "rgba(217,119,6,0.12)"],
  } as const;
  const [color, background] = map[tone];
  return (
    <span className="rounded px-1 font-semibold" style={{ color, background }}>
      {children}
    </span>
  );
}

// A meter card under the gateway: header (with optional balance) and a bar.
function MeterNode({
  w,
  icon,
  title,
  balance,
  barClass,
}: {
  w: number;
  icon: React.ReactNode;
  title: string;
  balance?: React.ReactNode;
  barClass: string;
}) {
  const h = 56;
  return (
    <foreignObject x={GX - w / 2} y={SINK_TOP} width={w} height={h} aria-hidden="true">
      <div className="flex h-full w-full flex-col justify-center gap-1.5 rounded-xl border border-fd-border bg-fd-card px-3 shadow-sm">
        <div className="flex items-center justify-between text-[13px] font-semibold text-fd-foreground">
          <span className="flex items-center gap-2">
            <span className="text-fd-muted-foreground">{icon}</span>
            {title}
          </span>
          {balance}
        </div>
        <div className="h-1.5 w-full overflow-hidden rounded-full bg-fd-muted">
          <div className={`h-full rounded-full bg-fd-primary ${barClass}`} />
        </div>
      </div>
    </foreignObject>
  );
}

// =====================================================================
// Provider catalogue
// =====================================================================

const ALL: Record<string, Provider> = {
  bedrock: {
    name: "Amazon Bedrock",
    node: <Bedrock.Color size={22} />,
    href: `${PROVIDERS_DOCS}#aws-bedrock`,
  },
  anthropic: {
    name: "Anthropic",
    node: <Anthropic size={20} style={{ color: "#D97757" }} />,
    href: `${PROVIDERS_DOCS}#anthropic`,
  },
  azure: {
    name: "Azure OpenAI",
    node: <AzureAI.Color size={22} />,
    href: `${PROVIDERS_DOCS}#azure-openai`,
  },
  gemini: {
    name: "Google Gemini",
    node: <Gemini.Color size={22} />,
    href: `${PROVIDERS_DOCS}#google-vertex-ai`,
  },
  openai: {
    name: "OpenAI",
    node: <OpenAI size={20} className="text-fd-foreground" />,
    href: `${PROVIDERS_DOCS}#openai`,
  },
  openrouter: {
    name: "OpenRouter",
    node: <OpenRouter size={20} style={{ color: "#6566F1" }} />,
    href: `${PROVIDERS_DOCS}#openai-compatible-providers`,
  },
  ollama: {
    name: "Ollama",
    node: <Ollama size={20} className="text-fd-foreground" />,
    href: `${PROVIDERS_DOCS}#openai-compatible-providers`,
  },
  onprem: {
    name: "On-prem",
    node: <Server size={18} strokeWidth={1.75} className="text-fd-muted-foreground" />,
    href: `${PROVIDERS_DOCS}#openai-compatible-providers`,
  },
  compatible: {
    name: "OpenAI-compatible",
    node: <Plug size={18} strokeWidth={1.75} className="text-fd-muted-foreground" />,
    href: `${PROVIDERS_DOCS}#openai-compatible-providers`,
  },
};

const ROUTING_SET = [
  ALL.bedrock,
  ALL.anthropic,
  ALL.azure,
  ALL.gemini,
  ALL.ollama,
  ALL.onprem,
  ALL.openai,
  ALL.compatible,
  ALL.openrouter,
];
const LEAN_SET = [ALL.anthropic, ALL.openai, ALL.gemini];
const LEAN_CADENCE = cadenceFor(LEAN_SET);

// =====================================================================
// Scenes
// =====================================================================

type Scene = {
  id: string;
  pill: string;
  caption: string;
  href: string;
  render: () => React.ReactNode;
};

function SinkWire() {
  return (
    <g fill="none" aria-hidden="true" className="stroke-fd-border" strokeWidth={1.5}>
      <path d={sinkPath} strokeDasharray="4 4" />
    </g>
  );
}

const scenes: Scene[] = [
  {
    id: "routing",
    pill: "Routing",
    caption:
      "One OpenAI-compatible API routes to any provider, with automatic fallbacks and health checks.",
    href: PROVIDERS_DOCS,
    render: () => (
      <>
        <Wires providers={ROUTING_SET} />
        <Lanes providers={ROUTING_SET} />
        <UserNode />
        <GatewayNode />
        <ProviderChips providers={ROUTING_SET} />
      </>
    ),
  },
  {
    id: "auth",
    pill: "Authentication",
    caption:
      "Every request is authenticated against your identity provider before it reaches a model.",
    href: "/docs/authentication",
    render: () => {
      const idpY = 116;
      const loginPath = `M${UX},${UY - 34} L${UX},${idpY + 22}`;
      const idpToGw = `M${UX + 80},${idpY} C ${UX + 190},${idpY} ${GW_LEFT - 70},${GY - 26} ${GW_LEFT - 4},${GY - 18}`;
      return (
        <>
          <Wires providers={LEAN_SET} />
          {/* IdP wired to the user (neutral) and the gateway (green: trusted identity). */}
          <g fill="none" aria-hidden="true" strokeWidth={1.5}>
            <path d={loginPath} strokeDasharray="4 4" className="stroke-fd-border" />
            <path d={idpToGw} strokeDasharray="4 4" className="stroke-emerald-500/70" />
          </g>
          {/* Requests turn green as they pass the gateway (authenticated). */}
          <Lanes providers={LEAN_SET} outClass="fill-emerald-500" />
          {/* Occasional, irregular login traffic to the IdP. */}
          <Flow path={loginPath} dur={5.3} begin={0.6} />
          <Flow path={loginPath} dur={7.1} begin={3.4} />
          <UserNode />
          <GatewayNode />
          <ProviderChips providers={LEAN_SET} />
          <SatelliteNode
            x={UX}
            y={idpY}
            w={150}
            h={44}
            icon={<Fingerprint className="h-5 w-5" />}
            title="Identity Provider"
          />
        </>
      );
    },
  },
  {
    id: "authz",
    pill: "Authorization",
    caption: "CEL-based RBAC evaluates system and org policies to allow or deny each request.",
    href: "/docs/features/authorization",
    render: () => (
      <>
        <Wires providers={LEAN_SET} />
        {/* Allowed requests continue green; denied ones turn red at the gateway. */}
        <Lanes providers={LEAN_SET} outClass="fill-emerald-500" />
        <ReturnDot
          path={userPath}
          peak={1}
          dur={LEAN_CADENCE * 4}
          begin={LEAN_CADENCE * 1.5}
          at={0.42}
          outClass="fill-red-500"
        />
        <UserNode />
        <GatewayNode />
        <ProviderChips providers={LEAN_SET} />
        <SinkWire />
        <RollingLog
          w={300}
          step={LEAN_CADENCE}
          title="Policy decisions"
          icon={<ShieldCheck className="h-4 w-4" />}
          rows={[
            <PolicyRow key="1" tone="allow" action="model:use" policy="org-member-read" />,
            <PolicyRow key="2" tone="deny" action="model:use" policy="premium-models" />,
            <PolicyRow key="3" tone="allow" action="vector_store:read" policy="org-member-read" />,
            <PolicyRow key="4" tone="deny" action="user:delete" policy="deny-self-delete" />,
            <PolicyRow key="5" tone="allow" action="responses:create" policy="org-admin" />,
          ]}
        />
      </>
    ),
  },
  {
    id: "rate-limits",
    pill: "Rate limiting",
    caption: "Per-key and per-tenant limits shed excess load before it reaches a provider.",
    href: "/docs/configuration/auth#per-key-rate-limits",
    render: () => (
      <>
        <Wires providers={LEAN_SET} />
        <Lanes providers={LEAN_SET} />
        {/* Once the bucket is full, requests reach the gateway then bounce back orange. */}
        <ReturnDot
          path={userPath}
          peak={1}
          dur={6.5}
          begin={0}
          at={0.73}
          outClass="fill-orange-500"
        />
        <UserNode />
        <GatewayNode />
        <ProviderChips providers={LEAN_SET} />
        <SinkWire />
        <MeterNode
          w={186}
          icon={<Gauge className="h-4 w-4" />}
          title="Rate limit"
          barClass="w-[60%] [animation:hadrian-rate_6.5s_linear_infinite] motion-reduce:[animation:none]"
        />
      </>
    ),
  },
  {
    id: "guardrails",
    pill: "Guardrails",
    caption:
      "Content moderation, PII detection, and virus scanning screen every request and response.",
    href: "/docs/features/guardrails",
    render: () => (
      <>
        <Wires providers={LEAN_SET} />
        {/* Successful requests pass through; blocked content turns away red. */}
        <Lanes providers={LEAN_SET} />
        <ReturnDot
          path={userPath}
          peak={1}
          dur={LEAN_CADENCE * 5}
          begin={LEAN_CADENCE * 2}
          at={0.4}
          outClass="fill-red-500"
        />
        <UserNode />
        <GatewayNode />
        <ProviderChips providers={LEAN_SET} />
        <SinkWire />
        {/* The guardrails provider and the screening log it produces, as one unit. */}
        <RollingLog
          w={224}
          step={LEAN_CADENCE}
          title="Guardrails"
          icon={<ShieldAlert className="h-4 w-4" />}
          href="/docs/features/guardrails"
          rows={[
            <ScreenRow key="1" tone="allow" label="passed" />,
            <ScreenRow key="2" tone="deny" label="pii_credit_card" />,
            <ScreenRow key="3" tone="allow" label="passed" />,
            <ScreenRow key="4" tone="deny" label="prompt_attack" />,
            <ScreenRow key="5" tone="redact" label="pii_email" />,
            <ScreenRow key="6" tone="allow" label="passed" />,
          ]}
        />
      </>
    ),
  },
  {
    id: "budgets",
    pill: "Budgets & cost",
    caption:
      "Scoped budgets and microcent cost tracking meter spend across orgs, teams, and projects.",
    href: "/docs/features/budgets",
    render: () => (
      <>
        <Wires providers={LEAN_SET} />
        {/* Requests vary in size to represent cost. */}
        <Lanes providers={LEAN_SET} sizes={[6.5, 3.5, 5]} />
        <UserNode />
        <GatewayNode />
        <ProviderChips providers={LEAN_SET} />
        <SinkWire />
        {/* Once the budget is spent, requests bounce back. */}
        <ReturnDot
          path={userPath}
          peak={1}
          dur={10}
          begin={0}
          at={0.83}
          outClass="fill-orange-500"
        />
        <MeterNode
          w={210}
          icon={<Wallet className="h-4 w-4" />}
          title="Budget"
          balance={
            <span className="hadrian-balance font-mono text-[11px] text-fd-muted-foreground [animation:hadrian-spend_10s_linear_infinite] motion-reduce:[animation:none]" />
          }
          barClass="w-[60%] [animation:hadrian-budget_10s_linear_infinite] motion-reduce:[animation:none]"
        />
      </>
    ),
  },
  {
    id: "caching",
    pill: "Caching",
    caption: "Redis-backed caching returns hits instantly, skipping the call to a provider.",
    href: "/docs/features/caching",
    render: () => {
      const ys = providerYs(4);
      const cacheY = ys[0];
      const llmYs = ys.slice(1);
      const cachePath = fullPath(cacheY);
      // The gateway -> cache leg runs at 2x speed (an instant hit). Solve the
      // travel fraction so the user -> gateway leg matches every other request.
      const g = gateFrac(cachePath);
      const L = pathLength(cachePath);
      const t1 = (g * L) / SPEED; // user -> gateway at SPEED
      const t2 = ((1 - g) * L) / (2 * SPEED); // gateway -> cache at 2x SPEED
      const cacheDur = LEAN_CADENCE * 3;
      const f1 = (t1 / cacheDur).toFixed(3);
      const f2 = ((t1 + t2) / cacheDur).toFixed(3);
      return (
        <>
          <g fill="none" aria-hidden="true" className="stroke-fd-border" strokeWidth={1.5}>
            <path d={userPath} />
            {ys.map((y, i) => (
              <path key={i} d={providerPath(y)} />
            ))}
          </g>
          {/* Misses route through to a provider (lanes sit on the lower 3 rows). */}
          {(() => {
            const paths = llmYs.map(fullPath);
            const period = lanePeriod(paths.map(travelTime));
            return paths.map((d, i) => {
              const begin = laneBegin(i, period);
              const at = Math.min(0.985, travelTime(d) / period);
              return (
                <g key={i}>
                  <NodeGlow x={PX} y={llmYs[i]} size={56} dur={period} begin={begin} at={at} />
                  <Flow path={d} dur={period} begin={begin} />
                </g>
              );
            });
          })()}
          {/* Cache lane: forward only, 2x speed on the gateway -> cache leg. */}
          {[0, 1].map((k) => (
            <circle
              key={k}
              r="4.5"
              className="fill-teal-500 motion-reduce:hidden"
              opacity={0}
              style={DOT_FILTER}
            >
              <animateMotion
                path={cachePath}
                dur={`${cacheDur}s`}
                begin={`${(k * cacheDur) / 2}s`}
                repeatCount="indefinite"
                calcMode="linear"
                keyPoints={`0;${g.toFixed(3)};1;1`}
                keyTimes={`0;${f1};${f2};1`}
              />
              <animate
                attributeName="opacity"
                values="0;1;1;0;0"
                keyTimes={`0;0.03;${(Number(f2) - 0.02).toFixed(3)};${(Number(f2) + 0.01).toFixed(3)};1`}
                dur={`${cacheDur}s`}
                begin={`${(k * cacheDur) / 2}s`}
                repeatCount="indefinite"
              />
            </circle>
          ))}
          <UserNode />
          <GatewayNode />
          <foreignObject
            x={PX - PROVIDER_HALF}
            y={cacheY - PROVIDER_HALF}
            width={VB_W - (PX - PROVIDER_HALF)}
            height={PROVIDER_HALF * 2}
          >
            <Link
              href="/docs/features/caching"
              aria-label="Caching documentation"
              className="group flex h-full items-center gap-3 no-underline"
            >
              <span className="flex aspect-square h-full flex-none items-center justify-center rounded-xl border border-dashed border-teal-500/60 bg-teal-500/5 shadow-sm">
                <Database className="h-5 w-5 text-teal-600 dark:text-teal-400" />
              </span>
              <span className="flex flex-col leading-tight">
                <span className="font-medium text-fd-foreground" style={{ fontSize: 14 }}>
                  Cache
                </span>
                <span className="text-[11px] text-teal-600 dark:text-teal-400">instant hit</span>
              </span>
            </Link>
          </foreignObject>
          {LEAN_SET.map((p, i) => (
            <Chip key={p.name} provider={p} y={llmYs[i]} />
          ))}
        </>
      );
    },
  },
  {
    id: "usage",
    pill: "Usage logging",
    caption:
      "Every request is logged with tokens, cost, and latency to the database, Prometheus, and OTLP.",
    href: "/docs/configuration/observability",
    render: () => (
      <>
        <Wires providers={LEAN_SET} />
        <Lanes providers={LEAN_SET} />
        <UserNode />
        <GatewayNode />
        <ProviderChips providers={LEAN_SET} />
        <SinkWire />
        <RollingLog
          w={252}
          step={LEAN_CADENCE}
          title="Usage log"
          icon={<ScrollText className="h-4 w-4" />}
          rows={[
            <UsageRow key="1" provider="openai" tok="1242 → 318" cost="$0.0023" lat="412ms" />,
            <UsageRow key="2" provider="anthropic" tok="880 → 1203" cost="$0.0142" lat="1.2s" />,
            <UsageRow key="3" provider="gemini" tok="512 → 240" cost="$0.0006" lat="380ms" />,
            <UsageRow key="4" provider="bedrock" tok="2048 → 96" cost="$0.0031" lat="540ms" />,
            <UsageRow key="5" provider="openai" tok="310 → 870" cost="$0.0089" lat="910ms" />,
          ]}
        />
      </>
    ),
  },
  {
    id: "sovereignty",
    pill: "Sovereignty",
    caption:
      "Requests route only to providers in compliant regions, based on their sovereignty rules.",
    href: "/docs/features/data-sovereignty",
    render: () => {
      const rows = [
        {
          p: ALL.openai,
          region: "US",
          fill: "fill-amber-500",
          stroke: "stroke-amber-500/70",
          dot: "#f59e0b",
        },
        {
          p: ALL.anthropic,
          region: "US",
          fill: "fill-amber-500",
          stroke: "stroke-amber-500/70",
          dot: "#f59e0b",
        },
        {
          p: ALL.azure,
          region: "EU",
          fill: "fill-blue-500",
          stroke: "stroke-blue-500/70",
          dot: "#3b82f6",
        },
        {
          p: ALL.onprem,
          region: "EU",
          fill: "fill-blue-500",
          stroke: "stroke-blue-500/70",
          dot: "#3b82f6",
        },
      ];
      const ys = providerYs(rows.length);
      const period = lanePeriod(ys.map((y) => travelTime(fullPath(y))));
      return (
        <>
          <g fill="none" aria-hidden="true" strokeWidth={1.5}>
            <path d={userPath} className="stroke-fd-border" />
            {rows.map((r, i) => (
              <path key={i} d={providerPath(ys[i])} className={r.stroke} />
            ))}
          </g>
          {/* Each request is one colour and travels to a single matching provider. */}
          {rows.map((r, i) => {
            const d = fullPath(ys[i]);
            const begin = laneBegin(i, period);
            const at = Math.min(0.985, travelTime(d) / period);
            return (
              <g key={r.p.name}>
                <NodeGlow x={PX} y={ys[i]} size={56} dur={period} begin={begin} at={at} />
                <Flow path={d} dur={period} begin={begin} className={r.fill} />
              </g>
            );
          })}
          <UserNode />
          <GatewayNode />
          {rows.map((r, i) => (
            <Chip key={r.p.name} provider={r.p} y={ys[i]} region={r.region} regionColor={r.dot} />
          ))}
        </>
      );
    },
  },
  {
    id: "tools",
    pill: "Server-side tools",
    caption:
      "The gateway runs MCP, shell, file search, and web search in an agentic loop, then routes.",
    href: "/docs/features/agents",
    render: () => {
      const tools = [
        { icon: <Plug className="h-4 w-4" />, label: "MCP", dur: 3.3, begin: 0.4 },
        { icon: <Terminal className="h-4 w-4" />, label: "Shell", dur: 4.1, begin: 1.9 },
        { icon: <FileSearch className="h-4 w-4" />, label: "Files", dur: 3.7, begin: 1.1 },
        { icon: <Globe className="h-4 w-4" />, label: "Web", dur: 4.7, begin: 2.6 },
      ];
      const startX = GX - 165;
      const gap = 110;
      const ty = SINK_TOP + 16;
      return (
        <>
          <Wires providers={LEAN_SET} />
          <Lanes providers={LEAN_SET} />
          <g fill="none" aria-hidden="true" className="stroke-fd-border" strokeWidth={1.5}>
            {tools.map((_, i) => (
              <path key={i} d={`M${GX},${GW_BOTTOM} L${startX + i * gap},${ty - 18}`} />
            ))}
          </g>
          <UserNode />
          <GatewayNode />
          {/* Incommensurate periods make the picked tool look random. */}
          {tools.map((t, i) => {
            const tx = startX + i * gap;
            const loop = `M${GX},${GW_BOTTOM} L${tx},${ty - 18} L${GX},${GW_BOTTOM}`;
            return (
              <g key={t.label}>
                <Flow path={loop} dur={t.dur} begin={t.begin} className="fill-violet-500" />
                <foreignObject x={tx - 42} y={ty - 16} width={84} height={34} aria-hidden="true">
                  <div className="flex h-full w-full items-center justify-center gap-1.5 rounded-lg border border-fd-border bg-fd-card text-[12px] font-medium text-fd-foreground shadow-sm">
                    <span className="text-fd-muted-foreground">{t.icon}</span>
                    {t.label}
                  </div>
                </foreignObject>
              </g>
            );
          })}
        </>
      );
    },
  },
];

function PolicyRow({
  tone,
  action,
  policy,
}: {
  tone: "allow" | "deny";
  action: string;
  policy: string;
}) {
  return (
    <>
      <Tag tone={tone}>{tone}</Tag>
      <span className="w-[116px] text-fd-foreground">{action}</span>
      <span className="ml-auto text-fd-muted-foreground">{policy}</span>
    </>
  );
}

function ScreenRow({ tone, label }: { tone: "allow" | "deny" | "redact"; label: string }) {
  return (
    <>
      <Tag tone={tone}>{tone}</Tag>
      <span className={tone === "allow" ? "text-fd-muted-foreground" : "text-fd-foreground"}>
        {label}
      </span>
    </>
  );
}

function UsageRow({
  provider,
  tok,
  cost,
  lat,
}: {
  provider: string;
  tok: string;
  cost: string;
  lat: string;
}) {
  return (
    <>
      <span className="w-[68px] text-fd-foreground">{provider}</span>
      <span className="w-[78px] text-fd-muted-foreground">{tok}</span>
      <span className="w-[54px] text-fd-foreground">{cost}</span>
      <span className="ml-auto text-fd-muted-foreground">{lat}</span>
    </>
  );
}

// =====================================================================
// Tabbed, auto-cycling wrapper
// =====================================================================

const REDUCED_MOTION_QUERY = "(prefers-reduced-motion: reduce)";

function usePrefersReducedMotion() {
  return useSyncExternalStore(
    (onChange) => {
      const mq = window.matchMedia(REDUCED_MOTION_QUERY);
      mq.addEventListener("change", onChange);
      return () => mq.removeEventListener("change", onChange);
    },
    () => window.matchMedia(REDUCED_MOTION_QUERY).matches,
    () => false
  );
}

const CYCLE_MS = 6500;

export function GatewayDiagram() {
  const [active, setActive] = useState(0);
  const [paused, setPaused] = useState(false);
  const reducedMotion = usePrefersReducedMotion();
  const tablistRef = useRef<HTMLDivElement>(null);

  const go = useCallback(
    (i: number) => setActive(((i % scenes.length) + scenes.length) % scenes.length),
    []
  );

  useEffect(() => {
    if (paused || reducedMotion) return;
    const id = window.setInterval(() => setActive((i) => (i + 1) % scenes.length), CYCLE_MS);
    return () => window.clearInterval(id);
  }, [paused, reducedMotion]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowRight" || e.key === "ArrowDown") {
      e.preventDefault();
      go(active + 1);
    } else if (e.key === "ArrowLeft" || e.key === "ArrowUp") {
      e.preventDefault();
      go(active - 1);
    } else if (e.key === "Home") {
      e.preventDefault();
      go(0);
    } else if (e.key === "End") {
      e.preventDefault();
      go(scenes.length - 1);
    }
  };

  const scene = scenes[active];

  return (
    <div
      className="flex flex-col items-center gap-5"
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      onFocusCapture={() => setPaused(true)}
      onBlurCapture={() => setPaused(false)}
    >
      <style>{`
        @keyframes hadrian-scene-fade { from { opacity: 0 } to { opacity: 1 } }
        @keyframes hadrian-rate {
          0% { width: 12% } 13% { width: 34% } 19% { width: 27% } 33% { width: 55% }
          39% { width: 47% } 55% { width: 80% } 61% { width: 71% } 75% { width: 100% }
          90% { width: 100% } 100% { width: 12% }
        }
        @keyframes hadrian-budget {
          0% { width: 6% } 8% { width: 6% } 10% { width: 15% } 24% { width: 15% }
          26% { width: 38% } 44% { width: 38% } 46% { width: 50% } 62% { width: 50% }
          64% { width: 82% } 80% { width: 82% } 82% { width: 100% } 95% { width: 100% }
          100% { width: 6% }
        }
        @property --hadrian-spent { syntax: "<integer>"; inherits: true; initial-value: 60; }
        @keyframes hadrian-spend {
          0% { --hadrian-spent: 60 } 8% { --hadrian-spent: 60 } 10% { --hadrian-spent: 150 }
          24% { --hadrian-spent: 150 } 26% { --hadrian-spent: 380 } 44% { --hadrian-spent: 380 }
          46% { --hadrian-spent: 500 } 62% { --hadrian-spent: 500 } 64% { --hadrian-spent: 820 }
          80% { --hadrian-spent: 820 } 82% { --hadrian-spent: 1000 } 95% { --hadrian-spent: 1000 }
          100% { --hadrian-spent: 60 }
        }
        .hadrian-balance::before {
          counter-reset: hs var(--hadrian-spent);
          content: "$" counter(hs) " / $1,000";
        }
        @keyframes hadrian-roll { from { transform: translateY(0) } to { transform: translateY(-50%) } }
        @media (prefers-reduced-motion: reduce) { .hadrian-noanim { animation: none !important } }
      `}</style>

      <div className="w-full overflow-x-auto">
        <div
          id={`gw-panel-${scene.id}`}
          role="tabpanel"
          aria-label={scene.pill}
          key={reducedMotion ? undefined : scene.id}
          style={reducedMotion ? undefined : { animation: "hadrian-scene-fade 420ms ease" }}
        >
          <svg
            viewBox={`0 0 ${VB_W} ${VB_H}`}
            aria-label={`Hadrian Gateway, ${scene.pill}. ${scene.caption}`}
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
            {scene.render()}
          </svg>
        </div>
      </div>

      <p className="flex min-h-[2.5rem] max-w-2xl flex-wrap items-center justify-center gap-x-1.5 text-center text-sm text-fd-muted-foreground">
        {scene.caption}{" "}
        <Link href={scene.href} className="whitespace-nowrap font-medium text-fd-primary">
          Learn more →
        </Link>
      </p>

      {/* Pill tabs switch the diagram (they do not navigate away). */}
      <div
        ref={tablistRef}
        role="tablist"
        aria-label="Gateway capabilities"
        onKeyDown={onKeyDown}
        className="flex max-w-3xl flex-wrap justify-center gap-2"
      >
        {scenes.map((s, i) => (
          <button
            key={s.id}
            type="button"
            role="tab"
            aria-selected={i === active}
            aria-controls={`gw-panel-${s.id}`}
            tabIndex={i === active ? 0 : -1}
            onClick={() => setActive(i)}
            onFocus={() => setActive(i)}
            className={`cursor-pointer rounded-full border px-3.5 py-1.5 text-sm font-medium transition-colors ${
              i === active
                ? "border-fd-primary bg-fd-primary text-fd-primary-foreground"
                : "border-fd-border bg-fd-card text-fd-muted-foreground hover:border-fd-primary/50 hover:text-fd-foreground"
            }`}
          >
            {s.pill}
          </button>
        ))}
      </div>
    </div>
  );
}
