"use client";

import { RotateCcw } from "lucide-react";
import { useCallback, useMemo, useReducer, useState } from "react";
import { type FieldProps, ObjectField, SchemaField, TaggedUnionField } from "./fields";
import {
  CONFIG_SECTIONS,
  type JsonSchema,
  SECTION_LABELS,
  fullyResolve,
  getPrimaryType,
  isTaggedUnion,
} from "./schema-utils";
import { generateToml } from "./toml-utils";
import { TomlPreview } from "./TomlPreview";
/* eslint-disable @typescript-eslint/no-explicit-any */

import configSchemaRaw from "../../public/config-schema.json";

const configSchema = configSchemaRaw as unknown as JsonSchema;

// ── Default starter config ─────────────────────────────────────────────────

const DEFAULT_STATE: Record<string, any> = {
  server: { host: "0.0.0.0", port: 8080 },
  database: { type: "sqlite", path: "./hadrian.db" },
  cache: { type: "memory" },
  providers: {
    default_provider: "ollama",
    ollama: { type: "open_ai", base_url: "http://localhost:11434/v1" },
  },
  ui: { enabled: true, chat: { enabled: true }, admin: { enabled: true } },
  docs: { enabled: true },
};

// ── State management ───────────────────────────────────────────────────────

type Action =
  | { type: "SET_VALUE"; section: string; value: any }
  | { type: "RESET_SECTION"; section: string }
  | { type: "RESET" };

function reducer(state: Record<string, any>, action: Action): Record<string, any> {
  switch (action.type) {
    case "SET_VALUE": {
      const next = { ...state };
      if (action.value === undefined || action.value === null) {
        delete next[action.section];
      } else {
        next[action.section] = action.value;
      }
      return next;
    }
    case "RESET_SECTION": {
      const next = { ...state };
      delete next[action.section];
      return next;
    }
    case "RESET":
      return DEFAULT_STATE;
  }
}

// ── Providers section (special handling) ───────────────────────────────────

const providerConfigDef = configSchema.definitions?.["ProviderConfig"];

function ProvidersSection({
  value,
  onChange,
  rootSchema,
}: {
  value: any;
  onChange: (v: any) => void;
  rootSchema: JsonSchema;
}) {
  const [newName, setNewName] = useState("");
  const current = useMemo(() => value ?? {}, [value]);

  const addProvider = useCallback(() => {
    const name = newName.trim().toLowerCase().replace(/\s+/g, "_");
    if (!name || name in current) return;
    onChange({ ...current, [name]: { type: "open_ai" } });
    setNewName("");
  }, [newName, current, onChange]);

  const removeProvider = useCallback(
    (name: string) => {
      const next = { ...current };
      delete next[name];
      onChange(Object.keys(next).length > 0 ? next : undefined);
    },
    [current, onChange]
  );

  const updateProvider = useCallback(
    (name: string, val: any) => {
      onChange({ ...current, [name]: val });
    },
    [current, onChange]
  );

  const updateDefaultProvider = useCallback(
    (val: any) => {
      onChange({ ...current, default_provider: val || undefined });
    },
    [current, onChange]
  );

  // Separate default_provider from named providers
  const { default_provider, ...providers } = current;
  const providerEntries = Object.entries(providers);

  return (
    <div className="space-y-4">
      <div>
        <label className="mb-1 block text-sm font-medium text-fd-foreground">
          Default Provider
        </label>
        <p className="mb-1.5 text-xs text-fd-muted-foreground">
          Default provider name for requests that don&apos;t specify one.
        </p>
        <input
          className="w-full rounded-md border border-fd-border bg-fd-card px-3 py-1.5 text-sm text-fd-foreground placeholder:text-fd-muted-foreground/50 focus:border-fd-primary focus:outline-none focus:ring-1 focus:ring-fd-primary"
          value={default_provider ?? ""}
          onChange={(e) => updateDefaultProvider(e.target.value)}
          placeholder="e.g. openai"
          aria-label="Default provider"
        />
      </div>

      <div className="border-t border-fd-border pt-4">
        <p className="mb-2 text-sm font-medium text-fd-foreground">Providers</p>
        <div className="mb-3 flex gap-2">
          <input
            className="w-full rounded-md border border-fd-border bg-fd-card px-3 py-1.5 text-sm text-fd-foreground placeholder:text-fd-muted-foreground/50 focus:border-fd-primary focus:outline-none focus:ring-1 focus:ring-fd-primary"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            placeholder="Provider name (e.g. openai, anthropic)"
            onKeyDown={(e) => e.key === "Enter" && addProvider()}
            aria-label="New provider name"
          />
          <button
            type="button"
            onClick={addProvider}
            disabled={!newName.trim()}
            className="shrink-0 rounded-md bg-fd-primary px-3 py-1.5 text-xs font-medium text-fd-primary-foreground transition-colors hover:bg-fd-primary/90 disabled:opacity-40"
            aria-label="Add provider"
          >
            Add
          </button>
        </div>

        {providerEntries.map(([name, val]) => (
          <div key={name} className="mb-3 rounded-md border border-fd-border bg-fd-muted/20 p-3">
            <div className="mb-2 flex items-center justify-between">
              <span className="font-mono text-sm font-medium text-fd-foreground">
                [providers.{name}]
              </span>
              <button
                type="button"
                onClick={() => removeProvider(name)}
                className="rounded-md px-2 py-0.5 text-xs text-fd-muted-foreground hover:bg-fd-muted hover:text-red-500"
                aria-label={`Remove provider ${name}`}
              >
                Remove
              </button>
            </div>
            {providerConfigDef ? (
              <SchemaField
                schema={providerConfigDef}
                value={val}
                onChange={(v) => updateProvider(name, v)}
                path={`providers.${name}`}
                rootSchema={rootSchema}
              />
            ) : (
              <p className="text-xs text-fd-muted-foreground">Provider schema not available</p>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

// ── Section Form (dispatches to correct field type) ────────────────────────

function SectionForm(props: Omit<FieldProps, "label"> & { description?: string }) {
  const { schema, rootSchema } = props;
  const resolved = fullyResolve(schema, rootSchema);

  // Tagged unions (database, cache, secrets, etc.) → SchemaField handles them
  if (isTaggedUnion(resolved)) {
    return <TaggedUnionField {...props} schema={resolved} hideLabel />;
  }

  // Regular objects → ObjectField with collapsible=false for top-level sections
  if (getPrimaryType(resolved) === "object" && resolved.properties) {
    return <ObjectField {...props} schema={resolved} collapsible={false} hideLabel />;
  }

  // Fallback
  return <SchemaField {...props} schema={resolved} />;
}

// ── Main Component ─────────────────────────────────────────────────────────

export function ConfigBuilder() {
  const [state, dispatch] = useReducer(reducer, DEFAULT_STATE);
  const [activeTab, setActiveTab] = useState<string>(CONFIG_SECTIONS[0]);

  const rootSchema = configSchema;

  const toml = useMemo(() => generateToml(state, rootSchema), [state, rootSchema]);

  const handleSectionChange = useCallback((section: string, value: any) => {
    dispatch({ type: "SET_VALUE", section, value });
  }, []);

  const handleResetSection = useCallback((section: string) => {
    dispatch({ type: "RESET_SECTION", section });
  }, []);

  const handleReset = useCallback(() => {
    dispatch({ type: "RESET" });
  }, []);

  // Resolve schema for the active section
  const sectionSchema = useMemo(() => {
    const prop = rootSchema.properties?.[activeTab];
    if (!prop) return null;
    return fullyResolve(prop, rootSchema);
  }, [rootSchema, activeTab]);

  const filledSections = useMemo(() => {
    const filled = new Set<string>();
    for (const section of CONFIG_SECTIONS) {
      if (state[section] && Object.keys(state[section]).length > 0) {
        filled.add(section);
      }
    }
    return filled;
  }, [state]);

  return (
    <div className="not-prose">
      {/* Tab bar */}
      <div className="mb-4 flex items-center gap-2 overflow-x-auto border-b border-fd-border pb-2 pt-1">
        <div className="flex flex-wrap gap-1.5">
          {CONFIG_SECTIONS.map((section) => (
            <button
              key={section}
              type="button"
              onClick={() => setActiveTab(section)}
              className={`relative rounded-md px-2.5 py-1 text-xs font-medium transition-colors sm:px-3 sm:py-1.5 sm:text-sm ${
                activeTab === section
                  ? "bg-fd-primary text-fd-primary-foreground"
                  : "bg-fd-muted text-fd-muted-foreground hover:text-fd-foreground"
              }`}
            >
              {SECTION_LABELS[section] ?? section}
              {filledSections.has(section) && (
                <span className="absolute -right-0.5 -top-0.5 h-1.5 w-1.5 rounded-full bg-green-500" />
              )}
            </button>
          ))}
        </div>
        <button
          type="button"
          onClick={handleReset}
          className="ml-auto shrink-0 rounded-md p-1.5 text-fd-muted-foreground transition-colors hover:bg-fd-muted hover:text-fd-foreground"
          aria-label="Reset all configuration"
          title="Reset all"
        >
          <RotateCcw className="h-4 w-4" />
        </button>
      </div>

      {/* Content: form + preview */}
      <div className="grid gap-4 lg:grid-cols-[1fr_minmax(0,_24rem)]">
        {/* Form panel */}
        <div className="min-w-0 rounded-lg border border-fd-border bg-fd-card p-4">
          <div className="mb-3 flex items-center justify-between">
            <div className="min-w-0">
              <h3 className="text-sm font-semibold text-fd-foreground">
                {SECTION_LABELS[activeTab] ?? activeTab}
              </h3>
              {sectionSchema?.description && (
                <p className="mt-0.5 text-xs text-fd-muted-foreground">
                  {sectionSchema.description}
                </p>
              )}
            </div>
            {filledSections.has(activeTab) && (
              <button
                type="button"
                onClick={() => handleResetSection(activeTab)}
                className="inline-flex shrink-0 items-center gap-1.5 rounded-md px-2 py-1 text-xs text-fd-muted-foreground transition-colors hover:bg-fd-muted hover:text-fd-foreground"
                aria-label={`Reset ${SECTION_LABELS[activeTab] ?? activeTab} section`}
              >
                <RotateCcw className="h-3 w-3" />
                Reset section
              </button>
            )}
          </div>
          {activeTab === "providers" ? (
            <ProvidersSection
              value={state.providers}
              onChange={(v) => handleSectionChange("providers", v)}
              rootSchema={rootSchema}
            />
          ) : sectionSchema ? (
            <SectionForm
              schema={sectionSchema}
              value={state[activeTab]}
              onChange={(v) => handleSectionChange(activeTab, v)}
              path={activeTab}
              rootSchema={rootSchema}
            />
          ) : (
            <p className="text-sm text-fd-muted-foreground">
              No schema available for this section.
            </p>
          )}
        </div>

        {/* Preview panel */}
        <div className="lg:sticky lg:top-16 lg:max-h-[calc(100vh-6rem)] lg:self-start">
          <TomlPreview toml={toml} />
        </div>
      </div>
    </div>
  );
}
