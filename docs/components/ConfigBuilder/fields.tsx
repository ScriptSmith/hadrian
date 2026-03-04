"use client";

import { ChevronDown, ChevronRight, Plus, Trash2 } from "lucide-react";
import { useCallback, useId, useMemo, useState } from "react";
import type { JsonSchema } from "./schema-utils";
import {
  fieldLabel,
  fullyResolve,
  getPrimaryType,
  getVariants,
  isSensitiveField,
  isTaggedUnion,
} from "./schema-utils";

/* eslint-disable @typescript-eslint/no-explicit-any */

export interface FieldProps {
  schema: JsonSchema;
  value: any;
  onChange: (value: any) => void;
  path: string;
  rootSchema: JsonSchema;
  label?: string;
}

// ── Shared styles ──────────────────────────────────────────────────────────

const inputClass =
  "w-full rounded-md border border-fd-border bg-fd-card px-3 py-1.5 text-sm text-fd-foreground placeholder:text-fd-muted-foreground/50 focus:border-fd-primary focus:outline-none focus:ring-1 focus:ring-fd-primary";

const selectClass =
  "w-full rounded-md border border-fd-border bg-fd-card px-3 py-1.5 text-sm text-fd-foreground focus:border-fd-primary focus:outline-none focus:ring-1 focus:ring-fd-primary";

// ── String Field ───────────────────────────────────────────────────────────

export function StringField({ schema, value, onChange, path, label }: FieldProps) {
  const id = useId();
  const sensitive = isSensitiveField(path.split(".").pop() ?? "");
  return (
    <div>
      <label htmlFor={id} className="mb-1 block text-sm font-medium text-fd-foreground">
        {label ?? fieldLabel(path.split(".").pop() ?? "")}
      </label>
      {schema.description && (
        <p className="mb-1.5 text-xs text-fd-muted-foreground">{schema.description}</p>
      )}
      <input
        id={id}
        type={sensitive ? "password" : "text"}
        className={inputClass}
        value={value ?? ""}
        onChange={(e) => onChange(e.target.value || undefined)}
        placeholder={schema.default != null ? String(schema.default) : undefined}
      />
    </div>
  );
}

// ── Number Field ───────────────────────────────────────────────────────────

export function NumberField({ schema, value, onChange, path, label }: FieldProps) {
  const id = useId();
  return (
    <div>
      <label htmlFor={id} className="mb-1 block text-sm font-medium text-fd-foreground">
        {label ?? fieldLabel(path.split(".").pop() ?? "")}
      </label>
      {schema.description && (
        <p className="mb-1.5 text-xs text-fd-muted-foreground">{schema.description}</p>
      )}
      <input
        id={id}
        type="number"
        className={inputClass}
        value={value ?? ""}
        onChange={(e) => onChange(e.target.value === "" ? undefined : Number(e.target.value))}
        placeholder={schema.default != null ? String(schema.default) : undefined}
        min={schema.minimum}
        max={schema.maximum}
      />
    </div>
  );
}

// ── Boolean Field ──────────────────────────────────────────────────────────

export function BooleanField({ schema, value, onChange, path, label }: FieldProps) {
  const id = useId();
  const checked = value === true;
  return (
    <div className="flex items-start gap-3">
      <button
        id={id}
        role="switch"
        type="button"
        aria-checked={checked}
        aria-label={label ?? fieldLabel(path.split(".").pop() ?? "")}
        className={`relative mt-0.5 h-5 w-9 shrink-0 rounded-full transition-colors ${
          checked ? "bg-fd-primary" : "bg-fd-border"
        }`}
        onClick={() => onChange(!checked)}
      >
        <span
          className={`absolute left-0.5 top-0.5 h-4 w-4 rounded-full bg-white transition-transform ${
            checked ? "translate-x-4" : "translate-x-0"
          }`}
        />
      </button>
      <div className="min-w-0">
        <label htmlFor={id} className="block text-sm font-medium text-fd-foreground">
          {label ?? fieldLabel(path.split(".").pop() ?? "")}
        </label>
        {schema.description && (
          <p className="text-xs text-fd-muted-foreground">{schema.description}</p>
        )}
      </div>
    </div>
  );
}

// ── Enum Field ─────────────────────────────────────────────────────────────

export function EnumField({ schema, value, onChange, path, label }: FieldProps) {
  const id = useId();
  const options = schema.enum ?? [];
  return (
    <div>
      <label htmlFor={id} className="mb-1 block text-sm font-medium text-fd-foreground">
        {label ?? fieldLabel(path.split(".").pop() ?? "")}
      </label>
      {schema.description && (
        <p className="mb-1.5 text-xs text-fd-muted-foreground">{schema.description}</p>
      )}
      <select
        id={id}
        className={selectClass}
        value={value ?? ""}
        onChange={(e) => onChange(e.target.value || undefined)}
      >
        <option value="">— Select —</option>
        {options.map((opt: any) => (
          <option key={String(opt)} value={String(opt)}>
            {String(opt)}
          </option>
        ))}
      </select>
    </div>
  );
}

// ── Tagged Union Field ─────────────────────────────────────────────────────

export function TaggedUnionField({
  schema,
  value,
  onChange,
  path,
  rootSchema,
  label,
  hideLabel,
}: FieldProps & { hideLabel?: boolean }) {
  const variants = getVariants(schema);
  const currentType = value?.type ?? "";

  const activeVariant = variants.find((v) => v.tag === currentType);

  const handleTypeChange = useCallback(
    (tag: string) => {
      if (tag === currentType) return;
      // Reset to just the type when switching variants
      onChange(tag ? { type: tag } : undefined);
    },
    [currentType, onChange]
  );

  const handleFieldChange = useCallback(
    (fieldName: string, fieldValue: any) => {
      const next = { ...(value ?? {}), type: currentType };
      if (fieldValue === undefined || fieldValue === null || fieldValue === "") {
        delete next[fieldName];
      } else {
        next[fieldName] = fieldValue;
      }
      onChange(next);
    },
    [value, currentType, onChange]
  );

  // Get non-type properties for the active variant
  const variantProps = activeVariant
    ? Object.entries(activeVariant.schema.properties ?? {}).filter(([k]) => k !== "type")
    : [];

  return (
    <div>
      {!hideLabel && (
        <p className="mb-1 text-sm font-medium text-fd-foreground">
          {label ?? fieldLabel(path.split(".").pop() ?? "")}
        </p>
      )}
      {!hideLabel && schema.description && (
        <p className="mb-2 text-xs text-fd-muted-foreground">{schema.description}</p>
      )}
      <div className="flex flex-wrap gap-1.5">
        {variants.map((v) => (
          <button
            key={v.tag}
            type="button"
            className={`rounded-md px-2.5 py-1 text-xs font-medium transition-colors ${
              currentType === v.tag
                ? "bg-fd-primary text-fd-primary-foreground"
                : "bg-fd-muted text-fd-muted-foreground hover:text-fd-foreground"
            }`}
            onClick={() => handleTypeChange(v.tag)}
          >
            {v.tag}
          </button>
        ))}
      </div>
      {activeVariant?.description && (
        <p className="mt-2 text-xs text-fd-muted-foreground">{activeVariant.description}</p>
      )}
      {variantProps.length > 0 && (
        <div className="mt-3 space-y-3 border-l-2 border-fd-border pl-4">
          {variantProps.map(([propName, propSchema]) => {
            const resolved = fullyResolve(propSchema, rootSchema);
            return (
              <SchemaField
                key={propName}
                schema={resolved}
                value={value?.[propName]}
                onChange={(v) => handleFieldChange(propName, v)}
                path={`${path}.${propName}`}
                rootSchema={rootSchema}
              />
            );
          })}
        </div>
      )}
    </div>
  );
}

// ── Array Field ────────────────────────────────────────────────────────────

export function ArrayField({ schema, value, onChange, path, rootSchema, label }: FieldProps) {
  const items: any[] = useMemo(() => (Array.isArray(value) ? value : []), [value]);
  const itemSchema = schema.items ? fullyResolve(schema.items, rootSchema) : { type: "string" };
  const isScalar =
    getPrimaryType(itemSchema) === "string" ||
    getPrimaryType(itemSchema) === "number" ||
    getPrimaryType(itemSchema) === "integer";

  const addItem = useCallback(() => {
    onChange([...items, isScalar ? "" : {}]);
  }, [items, isScalar, onChange]);

  const removeItem = useCallback(
    (index: number) => {
      const next = items.filter((_, i) => i !== index);
      onChange(next.length > 0 ? next : undefined);
    },
    [items, onChange]
  );

  const updateItem = useCallback(
    (index: number, val: any) => {
      const next = [...items];
      next[index] = val;
      onChange(next);
    },
    [items, onChange]
  );

  return (
    <div>
      <div className="mb-1 flex items-center justify-between">
        <p className="text-sm font-medium text-fd-foreground">
          {label ?? fieldLabel(path.split(".").pop() ?? "")}
        </p>
        <button
          type="button"
          onClick={addItem}
          className="inline-flex items-center gap-1 rounded-md px-2 py-0.5 text-xs font-medium text-fd-primary hover:bg-fd-muted"
          aria-label={`Add ${path.split(".").pop()} item`}
        >
          <Plus className="h-3 w-3" /> Add
        </button>
      </div>
      {schema.description && (
        <p className="mb-2 text-xs text-fd-muted-foreground">{schema.description}</p>
      )}
      {items.map((item, i) => (
        <div key={i} className="mb-2 flex items-start gap-2">
          <div className="min-w-0 flex-1">
            {isScalar ? (
              <input
                className={inputClass}
                value={item ?? ""}
                onChange={(e) => updateItem(i, e.target.value || undefined)}
                placeholder={`Item ${i + 1}`}
                aria-label={`${path.split(".").pop()} item ${i + 1}`}
              />
            ) : (
              <SchemaField
                schema={itemSchema}
                value={item}
                onChange={(v) => updateItem(i, v)}
                path={`${path}[${i}]`}
                rootSchema={rootSchema}
                label={`Item ${i + 1}`}
              />
            )}
          </div>
          <button
            type="button"
            onClick={() => removeItem(i)}
            className="mt-1.5 rounded-md p-1 text-fd-muted-foreground hover:bg-fd-muted hover:text-red-500"
            aria-label={`Remove item ${i + 1}`}
          >
            <Trash2 className="h-3.5 w-3.5" />
          </button>
        </div>
      ))}
    </div>
  );
}

// ── Map Field ──────────────────────────────────────────────────────────────

export function MapField({ schema, value, onChange, path, rootSchema, label }: FieldProps) {
  const entries = value && typeof value === "object" ? Object.entries(value) : [];
  const valueSchema =
    typeof schema.additionalProperties === "object"
      ? fullyResolve(schema.additionalProperties, rootSchema)
      : { type: "string" as const };
  const [newKey, setNewKey] = useState("");

  const addEntry = useCallback(() => {
    const key = newKey.trim();
    if (!key || (value && key in value)) return;
    onChange({ ...(value ?? {}), [key]: {} });
    setNewKey("");
  }, [newKey, value, onChange]);

  const removeEntry = useCallback(
    (key: string) => {
      const next = { ...(value ?? {}) };
      delete next[key];
      onChange(Object.keys(next).length > 0 ? next : undefined);
    },
    [value, onChange]
  );

  const updateEntry = useCallback(
    (key: string, val: any) => {
      onChange({ ...(value ?? {}), [key]: val });
    },
    [value, onChange]
  );

  return (
    <div>
      <p className="mb-1 text-sm font-medium text-fd-foreground">
        {label ?? fieldLabel(path.split(".").pop() ?? "")}
      </p>
      {schema.description && (
        <p className="mb-2 text-xs text-fd-muted-foreground">{schema.description}</p>
      )}
      <div className="mb-2 flex gap-2">
        <input
          className={inputClass}
          value={newKey}
          onChange={(e) => setNewKey(e.target.value)}
          placeholder="Entry name"
          onKeyDown={(e) => e.key === "Enter" && addEntry()}
          aria-label={`New ${path.split(".").pop()} entry name`}
        />
        <button
          type="button"
          onClick={addEntry}
          disabled={!newKey.trim()}
          className="inline-flex shrink-0 items-center gap-1 rounded-md bg-fd-primary px-3 py-1.5 text-xs font-medium text-fd-primary-foreground transition-colors hover:bg-fd-primary/90 disabled:opacity-40"
          aria-label={`Add ${path.split(".").pop()} entry`}
        >
          <Plus className="h-3 w-3" /> Add
        </button>
      </div>
      {entries.map(([key, val]) => (
        <div key={key} className="mb-3 rounded-md border border-fd-border bg-fd-muted/20 p-3">
          <div className="mb-2 flex items-center justify-between">
            <span className="text-sm font-mono font-medium text-fd-foreground">{key}</span>
            <button
              type="button"
              onClick={() => removeEntry(key)}
              className="rounded-md p-1 text-fd-muted-foreground hover:bg-fd-muted hover:text-red-500"
              aria-label={`Remove ${key}`}
            >
              <Trash2 className="h-3.5 w-3.5" />
            </button>
          </div>
          <SchemaField
            schema={valueSchema}
            value={val}
            onChange={(v) => updateEntry(key, v)}
            path={`${path}.${key}`}
            rootSchema={rootSchema}
          />
        </div>
      ))}
    </div>
  );
}

// ── Object Field ───────────────────────────────────────────────────────────

export function ObjectField({
  schema,
  value,
  onChange,
  path,
  rootSchema,
  label,
  collapsible = true,
  hideLabel,
}: FieldProps & { collapsible?: boolean; hideLabel?: boolean }) {
  const [open, setOpen] = useState(!collapsible);
  const properties = schema.properties ?? {};
  const propEntries = Object.entries(properties);

  const handleFieldChange = useCallback(
    (fieldName: string, fieldValue: any) => {
      const next = { ...(value ?? {}) };
      if (fieldValue === undefined || fieldValue === null) {
        delete next[fieldName];
      } else {
        next[fieldName] = fieldValue;
      }
      const hasValues = Object.keys(next).length > 0;
      onChange(hasValues ? next : undefined);
    },
    [value, onChange]
  );

  if (propEntries.length === 0) return null;

  const showHeader = !hideLabel;

  const header = showHeader ? (
    <div className="flex items-center gap-2">
      {collapsible &&
        (open ? (
          <ChevronDown className="h-4 w-4 text-fd-muted-foreground" />
        ) : (
          <ChevronRight className="h-4 w-4 text-fd-muted-foreground" />
        ))}
      <span className="text-sm font-medium text-fd-foreground">
        {label ?? fieldLabel(path.split(".").pop() ?? "")}
      </span>
    </div>
  ) : null;

  return (
    <div>
      {showHeader &&
        (collapsible ? (
          <button
            type="button"
            onClick={() => setOpen(!open)}
            className="mb-1 w-full text-left"
            aria-expanded={open}
          >
            {header}
            {schema.description && (
              <p className="ml-6 text-xs text-fd-muted-foreground">{schema.description}</p>
            )}
          </button>
        ) : (
          <>
            {header}
            {schema.description && (
              <p className="mb-2 text-xs text-fd-muted-foreground">{schema.description}</p>
            )}
          </>
        ))}
      {open && (
        <div className={`space-y-3 ${collapsible ? "border-l-2 border-fd-border pl-4 pt-2" : ""}`}>
          {propEntries.map(([propName, propSchema]) => {
            const resolved = fullyResolve(propSchema, rootSchema);
            return (
              <SchemaField
                key={propName}
                schema={resolved}
                value={value?.[propName]}
                onChange={(v) => handleFieldChange(propName, v)}
                path={`${path}.${propName}`}
                rootSchema={rootSchema}
              />
            );
          })}
        </div>
      )}
    </div>
  );
}

// ── Schema Field (dispatcher) ──────────────────────────────────────────────

export function SchemaField(props: FieldProps) {
  const { schema, rootSchema } = props;
  const resolved = fullyResolve(schema, rootSchema);
  const type = getPrimaryType(resolved);

  // Tagged union (oneOf with type discriminator)
  if (isTaggedUnion(resolved)) {
    return <TaggedUnionField {...props} schema={resolved} />;
  }

  // Object with only additionalProperties (map)
  if (
    type === "object" &&
    resolved.additionalProperties &&
    typeof resolved.additionalProperties === "object" &&
    Object.keys(resolved.properties ?? {}).length === 0
  ) {
    return <MapField {...props} schema={resolved} />;
  }

  // Object with properties
  if (type === "object" && resolved.properties) {
    return <ObjectField {...props} schema={resolved} />;
  }

  // String enum
  if ((type === "string" || !type) && resolved.enum) {
    return <EnumField {...props} schema={resolved} />;
  }

  // String
  if (type === "string") {
    return <StringField {...props} schema={resolved} />;
  }

  // Number / integer
  if (type === "integer" || type === "number") {
    return <NumberField {...props} schema={resolved} />;
  }

  // Boolean
  if (type === "boolean") {
    return <BooleanField {...props} schema={resolved} />;
  }

  // Array
  if (type === "array") {
    return <ArrayField {...props} schema={resolved} />;
  }

  // Fallback: render as string input
  return <StringField {...props} schema={resolved} />;
}
