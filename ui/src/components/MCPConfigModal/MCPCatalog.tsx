/**
 * MCPCatalog — browse and add MCP servers from the official registry.
 *
 * Presents two sections:
 *   - "Connect directly" — remote servers (streamable-http / SSE)
 *   - "Run locally" — servers that only ship stdio packages; user runs a
 *     local proxy (e.g. npx mcp-remote) and connects over localhost.
 *
 * Clicking "Add" on a card hands a prefill payload back to the parent modal,
 * which opens the existing add-server form populated from the registry entry.
 */

import { useEffect, useRef, useState } from "react";
import {
  AlertCircle,
  ArrowLeft,
  ExternalLink,
  Globe,
  Link2,
  Loader2,
  Package,
  Plug,
  Plus,
  Search,
  Star,
} from "lucide-react";

import { Button } from "@/components/Button/Button";
import { Input } from "@/components/Input/Input";
import { cn } from "@/utils/cn";
import { useDebouncedValue } from "@/hooks/useDebouncedValue";
import {
  buildInstallCommand,
  categorize,
  dedupeLatest,
  getRegistryEntry,
  materializeHeaders,
  pickPreferredPackage,
  searchRegistry,
  type CategorizedEntry,
} from "@/services/mcpRegistry/client";
import type {
  MCPRegistryEntry,
  MCPRegistryPackage,
  MCPRegistryRemote,
} from "@/services/mcpRegistry/types";
import type { FavoriteMcpServer } from "@/config/types";

const PAGE_SIZE = 30;

export interface CatalogPrefill {
  url: string;
  name?: string;
  authType?: "none" | "bearer";
  bearerToken?: string;
  headers?: Record<string, string>;
  localInstall?: {
    command: string;
    envVars: Array<{
      name: string;
      description?: string;
      isSecret?: boolean;
      isRequired?: boolean;
    }>;
  };
}

export interface MCPCatalogProps {
  onPick: (prefill: CatalogPrefill) => void;
  onAddManual: () => void;
  onCancel: () => void;
  /**
   * Curated favorites shown at the top of the catalog. Each entry's `url` is
   * either a direct remote URL (`https://…`) or a registry identifier the
   * component resolves against the MCP registry.
   */
  favorites?: FavoriteMcpServer[];
}

/** Does this favorite's `url` look like a direct HTTP(S) URL? */
function isHttpUrl(value: string): boolean {
  try {
    const u = new URL(value);
    return u.protocol === "http:" || u.protocol === "https:";
  } catch {
    return false;
  }
}

/**
 * Resolution state for a single favorite.
 * URL favorites don't need resolution — they're rendered directly.
 * Registry favorites go loading → resolved | error as their registry lookup
 * completes.
 */
type FavoriteResolution =
  | { kind: "url" }
  | { kind: "loading" }
  | { kind: "resolved"; categorized: CategorizedEntry }
  | { kind: "error" };

export function MCPCatalog({ onPick, onAddManual, onCancel, favorites = [] }: MCPCatalogProps) {
  const [query, setQuery] = useState("");
  const [pasteUrl, setPasteUrl] = useState("");
  const debouncedQuery = useDebouncedValue(query, 300);
  const pasteUrlIsValid = (() => {
    try {
      const u = new URL(pasteUrl);
      return u.protocol === "http:" || u.protocol === "https:";
    } catch {
      return false;
    }
  })();

  const handlePasteSubmit = () => {
    if (!pasteUrlIsValid) return;
    onPick({ url: pasteUrl });
  };
  const [entries, setEntries] = useState<MCPRegistryEntry[]>([]);
  const [cursor, setCursor] = useState<string | undefined>();
  const [loading, setLoading] = useState(false);
  // `loadingMore` tracks which section's "Load more" button was clicked so we
  // only show a spinner on that button. The registry has a single shared
  // cursor — we loop fetching pages until an entry of the desired kind appears.
  const [loadingMore, setLoadingMore] = useState<"remote" | "local" | null>(null);
  const [error, setError] = useState<string | undefined>();
  const abortRef = useRef<AbortController | null>(null);

  // Resolution state for registry-ID favorites. URL favorites aren't tracked
  // here since they don't need resolving.
  const [favoriteResolutions, setFavoriteResolutions] = useState<Map<string, FavoriteResolution>>(
    () => new Map()
  );

  // Stable dependency key so the effect only re-runs when the favorites list
  // actually changes (by reference values, not array identity).
  const favoritesKey = favorites.map((f) => `${f.name}|${f.url}`).join("\n");

  useEffect(() => {
    const ctrl = new AbortController();

    // Seed URL favorites immediately; they render without a fetch.
    const seed = new Map<string, FavoriteResolution>();
    for (const f of favorites) {
      seed.set(f.url, isHttpUrl(f.url) ? { kind: "url" } : { kind: "loading" });
    }
    setFavoriteResolutions(seed);

    const registryFavorites = favorites.filter((f) => !isHttpUrl(f.url));
    if (registryFavorites.length === 0) return () => ctrl.abort();

    Promise.all(
      registryFavorites.map((f) =>
        getRegistryEntry(f.url, ctrl.signal)
          .then((entry): [string, FavoriteResolution] => {
            const c = categorize(entry);
            return [f.url, c ? { kind: "resolved", categorized: c } : { kind: "error" }];
          })
          .catch((): [string, FavoriteResolution] => [f.url, { kind: "error" }])
      )
    ).then((pairs) => {
      if (ctrl.signal.aborted) return;
      setFavoriteResolutions((prev) => {
        const next = new Map(prev);
        for (const [k, v] of pairs) next.set(k, v);
        return next;
      });
    });

    return () => ctrl.abort();
    // favoritesKey captures the meaningful identity of `favorites` for the effect.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [favoritesKey]);

  const hasFavorites = favorites.length > 0;

  // Initial load + search
  useEffect(() => {
    abortRef.current?.abort();
    const ctrl = new AbortController();
    abortRef.current = ctrl;

    setLoading(true);
    setError(undefined);

    searchRegistry({
      search: debouncedQuery || undefined,
      limit: PAGE_SIZE,
      signal: ctrl.signal,
    })
      .then((res) => {
        if (ctrl.signal.aborted) return;
        setEntries(res.servers);
        setCursor(res.metadata?.nextCursor);
      })
      .catch((err: unknown) => {
        if (ctrl.signal.aborted || (err instanceof DOMException && err.name === "AbortError"))
          return;
        setError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (!ctrl.signal.aborted) setLoading(false);
      });

    return () => ctrl.abort();
  }, [debouncedQuery]);

  const handleLoadMore = async (kind: "remote" | "local") => {
    if (!cursor || loadingMore) return;
    // Share the main search effect's abort controller so a new search mid-load
    // cancels this loop — otherwise stale pages from the previous query would
    // be appended on top of the fresh results.
    const ctrl = abortRef.current;
    setLoadingMore(kind);
    let nextCursor: string | undefined = cursor;
    // The registry returns mixed remote/local entries under one cursor, and
    // stdio-only servers are rare. Keep fetching pages until we either pick
    // up at least one new entry of the requested kind or run out of pages.
    const maxIterations = 10;
    try {
      for (let i = 0; i < maxIterations; i++) {
        if (ctrl?.signal.aborted) return;
        const res = await searchRegistry({
          search: debouncedQuery || undefined,
          limit: PAGE_SIZE,
          cursor: nextCursor,
          signal: ctrl?.signal,
        });
        if (ctrl?.signal.aborted) return;
        setEntries((prev) => [...prev, ...res.servers]);
        nextCursor = res.metadata?.nextCursor;

        const matched = res.servers.some((e) => {
          const c = categorize(e);
          return c?.kind === kind;
        });
        if (matched || !nextCursor) break;
      }
      if (!ctrl?.signal.aborted) setCursor(nextCursor);
    } catch (err) {
      if (ctrl?.signal.aborted || (err instanceof DOMException && err.name === "AbortError"))
        return;
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      // Always clear, even on abort — otherwise a new search cancelling an
      // in-flight load-more would leave the button stuck in its loading state.
      setLoadingMore(null);
    }
  };

  const categorized = dedupeLatest(entries)
    .map(categorize)
    .filter((c): c is CategorizedEntry => c != null);
  const remoteEntries = categorized.filter((c) => c.kind === "remote");
  const localEntries = categorized.filter((c) => c.kind === "local");

  return (
    <div className="space-y-5">
      {/* Prominent paste-URL card — direct-add for servers not in the registry */}
      <div className="rounded-lg border bg-muted/30 p-4 space-y-2">
        <div className="flex items-center gap-2">
          <Link2 className="h-4 w-4 text-primary" />
          <span className="text-sm font-medium">Have a URL? Add it directly</span>
        </div>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            handlePasteSubmit();
          }}
          className="flex gap-2"
        >
          <Input
            value={pasteUrl}
            onChange={(e) => setPasteUrl(e.target.value)}
            placeholder="https://mcp.example.com"
            className="flex-1 font-mono text-sm"
            aria-label="Paste MCP server URL"
            type="url"
          />
          <Button type="submit" disabled={!pasteUrlIsValid}>
            <Plus className="h-4 w-4" />
            Add
          </Button>
        </form>
        <div className="text-xs text-muted-foreground">
          Or{" "}
          <button type="button" onClick={onAddManual} className="underline hover:text-foreground">
            add manually with full configuration
          </button>
          .
        </div>
      </div>

      {hasFavorites && (
        <div className="space-y-2">
          <div>
            <div className="flex items-center gap-1.5 text-xs font-medium text-muted-foreground uppercase tracking-wider">
              <Star className="h-3.5 w-3.5" />
              Favorites
            </div>
            <p className="text-xs text-muted-foreground mt-0.5">
              Curated MCP servers recommended by your gateway admin.
            </p>
          </div>
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-3">
            {favorites.map((fav) => {
              const resolution = favoriteResolutions.get(fav.url) ?? { kind: "loading" };
              return (
                <FavoriteCard
                  key={`${fav.name}|${fav.url}`}
                  fav={fav}
                  resolution={resolution}
                  onPick={onPick}
                />
              );
            })}
          </div>
        </div>
      )}

      {/* Browse the registry */}
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <div className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
            Browse the registry
          </div>
          <a
            href="https://registry.modelcontextprotocol.io"
            target="_blank"
            rel="noreferrer"
            className="text-xs text-muted-foreground underline hover:text-foreground inline-flex items-center gap-1"
          >
            registry.modelcontextprotocol.io
            <ExternalLink className="h-3 w-3" />
          </a>
        </div>
        <div className="relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground pointer-events-none" />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search the MCP registry (e.g. github, atlassian)"
            className="pl-9"
            aria-label="Search MCP registry"
          />
          {loading && (
            <Loader2 className="absolute right-3 top-1/2 -translate-y-1/2 h-4 w-4 animate-spin text-muted-foreground" />
          )}
        </div>
      </div>

      {error && (
        <div className="flex items-start gap-2 text-sm text-destructive bg-destructive/10 p-3 rounded-md">
          <AlertCircle className="h-4 w-4 shrink-0 mt-0.5" />
          <div className="flex-1">
            <div className="font-medium">Could not load registry</div>
            <div className="text-xs mt-0.5">{error}</div>
          </div>
        </div>
      )}

      {!error && !loading && categorized.length === 0 && (
        <div className="text-center py-8 text-muted-foreground">
          <Plug className="h-8 w-8 mx-auto mb-3 opacity-50" />
          <p className="text-sm">No servers match &ldquo;{debouncedQuery}&rdquo;</p>
          <p className="text-xs mt-1">Try a different search or add one manually.</p>
        </div>
      )}

      {remoteEntries.length > 0 && (
        <Section
          title="Connect directly"
          icon={<Globe className="h-3.5 w-3.5" />}
          hint="Remote servers reachable over HTTP — no install needed."
          footer={
            cursor && !loading ? (
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => handleLoadMore("remote")}
                isLoading={loadingMore === "remote"}
                disabled={loadingMore !== null}
              >
                Load more
              </Button>
            ) : null
          }
        >
          {remoteEntries.map((c) => (
            <RemoteCard
              key={c.entry.server.name}
              entry={c.entry}
              remotes={c.remotes}
              onPick={onPick}
            />
          ))}
        </Section>
      )}

      {localEntries.length > 0 && (
        <Section
          title="Run locally"
          icon={<Package className="h-3.5 w-3.5" />}
          hint="Stdio-only servers. You'll install a package and run a proxy like npx mcp-remote, then connect over localhost."
          footer={
            cursor && !loading ? (
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => handleLoadMore("local")}
                isLoading={loadingMore === "local"}
                disabled={loadingMore !== null}
              >
                Load more
              </Button>
            ) : null
          }
        >
          {localEntries.map((c) => (
            <LocalCard
              key={c.entry.server.name}
              entry={c.entry}
              packages={c.packages}
              onPick={onPick}
            />
          ))}
        </Section>
      )}

      <div className="flex justify-start pt-2 border-t">
        <Button type="button" variant="ghost" onClick={onCancel}>
          <ArrowLeft className="h-4 w-4 mr-1.5" />
          Back
        </Button>
      </div>
    </div>
  );
}

function Section({
  title,
  icon,
  hint,
  footer,
  children,
}: {
  title: string;
  icon: React.ReactNode;
  hint: string;
  footer?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-2">
      <div>
        <div className="flex items-center gap-1.5 text-xs font-medium text-muted-foreground uppercase tracking-wider">
          {icon}
          {title}
        </div>
        <p className="text-xs text-muted-foreground mt-0.5">{hint}</p>
      </div>
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-3">{children}</div>
      {footer && <div className="flex justify-center pt-1">{footer}</div>}
    </div>
  );
}

function registryEntryUrl(name: string): string {
  return `https://registry.modelcontextprotocol.io/v0.1/servers/${encodeURIComponent(name)}/versions/latest`;
}

function ServerHeader({ entry }: { entry: MCPRegistryEntry }) {
  const [expanded, setExpanded] = useState(false);
  const title = entry.server.title || entry.server.name;
  const iconSrc = entry.server.icons?.[0]?.src;
  const description = entry.server.description;
  // Rough truncation check — 2 lines at this width is ~120 chars. Under that we
  // don't bother showing the "more" toggle.
  const isLongDescription = (description?.length ?? 0) > 120;

  return (
    <div className="flex items-start gap-3 min-w-0 flex-1">
      <div className="shrink-0 h-9 w-9 rounded-md bg-muted flex items-center justify-center overflow-hidden">
        {iconSrc ? (
          <img src={iconSrc} alt="" className="h-9 w-9 object-cover" />
        ) : (
          <Plug className="h-4 w-4 text-muted-foreground" />
        )}
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="font-medium text-sm truncate">{title}</span>
          {entry.server.version && (
            <span className="text-[10px] font-mono text-muted-foreground">
              v{entry.server.version}
            </span>
          )}
        </div>
        <div className="text-xs text-muted-foreground truncate">{entry.server.name}</div>
        {description && (
          <div className="mt-1">
            <p
              className={cn(
                "text-xs text-muted-foreground",
                !expanded && isLongDescription && "line-clamp-2"
              )}
            >
              {description}
            </p>
            {isLongDescription && (
              <button
                type="button"
                onClick={() => setExpanded(!expanded)}
                className="text-[11px] text-muted-foreground hover:text-foreground underline mt-0.5"
              >
                {expanded ? "Show less" : "Show more"}
              </button>
            )}
          </div>
        )}
        <div className="flex items-center gap-3 mt-1.5 flex-wrap">
          {entry.server.repository?.url && (
            <a
              href={entry.server.repository.url}
              target="_blank"
              rel="noreferrer"
              className="inline-flex items-center gap-1 text-[11px] text-muted-foreground hover:text-foreground underline"
              onClick={(e) => e.stopPropagation()}
            >
              {entry.server.repository.url.replace(/^https?:\/\//, "")}
              <ExternalLink className="h-2.5 w-2.5" />
            </a>
          )}
          <a
            href={registryEntryUrl(entry.server.name)}
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-1 text-[11px] text-muted-foreground hover:text-foreground underline"
            onClick={(e) => e.stopPropagation()}
            title="View the registry entry JSON"
          >
            registry entry
            <ExternalLink className="h-2.5 w-2.5" />
          </a>
        </div>
      </div>
    </div>
  );
}

function pickPreferredRemote(remotes: MCPRegistryRemote[]): MCPRegistryRemote {
  return remotes.find((r) => r.type === "streamable-http") ?? remotes[0];
}

function RemoteCard({
  entry,
  remotes,
  onPick,
}: {
  entry: MCPRegistryEntry;
  remotes: MCPRegistryRemote[];
  onPick: (p: CatalogPrefill) => void;
}) {
  const remote = pickPreferredRemote(remotes);
  const headers = materializeHeaders(remote);

  // Infer a bearer token from a templated Authorization header if present.
  let authType: CatalogPrefill["authType"] = "none";
  let bearerToken: string | undefined;
  const authValue = headers.Authorization ?? headers.authorization;
  const remainingHeaders: Record<string, string> = { ...headers };
  if (authValue && /^Bearer\s+/i.test(authValue)) {
    authType = "bearer";
    bearerToken = authValue.replace(/^Bearer\s+/i, "");
    delete remainingHeaders.Authorization;
    delete remainingHeaders.authorization;
  }

  const handleAdd = () => {
    onPick({
      url: remote.url,
      name: entry.server.title || entry.server.name,
      authType,
      bearerToken,
      headers: Object.keys(remainingHeaders).length > 0 ? remainingHeaders : undefined,
    });
  };

  return (
    <div className="border rounded-lg p-3 flex items-start gap-3">
      <ServerHeader entry={entry} />
      <div className="shrink-0 flex flex-col items-end gap-2">
        <div className="flex gap-1">
          {remotes.map((r, i) => (
            <span
              key={`${r.type}-${i}`}
              className={cn(
                "text-[10px] font-mono px-1.5 py-0.5 rounded",
                r.type === "streamable-http"
                  ? "bg-primary/10 text-primary"
                  : "bg-muted text-muted-foreground"
              )}
            >
              {r.type}
            </span>
          ))}
        </div>
        <Button type="button" size="sm" onClick={handleAdd}>
          <Plus className="h-3.5 w-3.5" />
          Add
        </Button>
      </div>
    </div>
  );
}

function LocalCard({
  entry,
  packages,
  onPick,
}: {
  entry: MCPRegistryEntry;
  packages: MCPRegistryPackage[];
  onPick: (p: CatalogPrefill) => void;
}) {
  const pkg = pickPreferredPackage(packages);
  const install = pkg ? buildInstallCommand(pkg) : null;

  const handleAdd = () => {
    if (!pkg || !install) return;
    onPick({
      url: install.url,
      name: entry.server.title || entry.server.name,
      authType: "none",
      localInstall: {
        command: install.command,
        envVars: (pkg.environmentVariables ?? []).map((e) => ({
          name: e.name,
          description: e.description,
          isSecret: e.isSecret,
          isRequired: e.isRequired,
        })),
      },
    });
  };

  return (
    <div className="border rounded-lg p-3 flex items-start gap-3">
      <ServerHeader entry={entry} />
      <div className="shrink-0 flex flex-col items-end gap-2">
        <div className="flex gap-1 flex-wrap justify-end">
          {Array.from(new Set(packages.map((p) => p.registryType))).map((type) => (
            <span
              key={type}
              className={cn(
                "text-[10px] font-mono px-1.5 py-0.5 rounded",
                type === pkg?.registryType
                  ? "bg-primary/10 text-primary"
                  : "bg-muted text-muted-foreground"
              )}
            >
              {type}
            </span>
          ))}
        </div>
        <Button type="button" size="sm" variant="outline" onClick={handleAdd} disabled={!install}>
          <Plus className="h-3.5 w-3.5" />
          Set up
        </Button>
      </div>
    </div>
  );
}

/**
 * Renders a single favorite entry. URL favorites get a compact card; registry
 * favorites delegate to the same RemoteCard / LocalCard components used for
 * search results once the registry entry has resolved.
 */
function FavoriteCard({
  fav,
  resolution,
  onPick,
}: {
  fav: FavoriteMcpServer;
  resolution: FavoriteResolution;
  onPick: (p: CatalogPrefill) => void;
}) {
  if (resolution.kind === "url") {
    const handleAdd = () => onPick({ url: fav.url, name: fav.name });
    return (
      <div className="border rounded-lg p-3 flex items-start gap-3">
        <div className="flex items-start gap-3 min-w-0 flex-1">
          <div className="shrink-0 h-9 w-9 rounded-md bg-muted flex items-center justify-center">
            <Plug className="h-4 w-4 text-muted-foreground" />
          </div>
          <div className="flex-1 min-w-0">
            <div className="font-medium text-sm truncate">{fav.name}</div>
            <div className="text-xs text-muted-foreground font-mono truncate">{fav.url}</div>
          </div>
        </div>
        <div className="shrink-0">
          <Button type="button" size="sm" onClick={handleAdd}>
            <Plus className="h-3.5 w-3.5" />
            Add
          </Button>
        </div>
      </div>
    );
  }

  if (resolution.kind === "resolved") {
    const c = resolution.categorized;
    if (c.kind === "remote") {
      return <RemoteCard entry={c.entry} remotes={c.remotes} onPick={onPick} />;
    }
    return <LocalCard entry={c.entry} packages={c.packages} onPick={onPick} />;
  }

  // loading / error — show a minimal card so the layout doesn't shift.
  return (
    <div className="border rounded-lg p-3 flex items-center gap-3">
      <div className="shrink-0 h-9 w-9 rounded-md bg-muted flex items-center justify-center">
        {resolution.kind === "loading" ? (
          <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
        ) : (
          <AlertCircle className="h-4 w-4 text-muted-foreground" />
        )}
      </div>
      <div className="flex-1 min-w-0">
        <div className="font-medium text-sm truncate">{fav.name}</div>
        <div className="text-xs text-muted-foreground truncate">
          {resolution.kind === "loading" ? fav.url : `Could not load ${fav.url}`}
        </div>
      </div>
    </div>
  );
}
