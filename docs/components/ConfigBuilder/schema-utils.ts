/* eslint-disable @typescript-eslint/no-explicit-any */

export interface JsonSchema {
  $ref?: string;
  $schema?: string;
  title?: string;
  description?: string;
  type?: string | string[];
  properties?: Record<string, JsonSchema>;
  additionalProperties?: JsonSchema | boolean;
  required?: string[];
  default?: any;
  enum?: any[];
  const?: any;
  oneOf?: JsonSchema[];
  anyOf?: JsonSchema[];
  allOf?: JsonSchema[];
  items?: JsonSchema;
  definitions?: Record<string, JsonSchema>;
  format?: string;
  minimum?: number;
  maximum?: number;
  nullable?: boolean;
}

export function resolveRef(schema: JsonSchema, root: JsonSchema): JsonSchema {
  if (!schema.$ref) return schema;
  const path = schema.$ref.replace(/^#\//, "").split("/");
  let current: any = root;
  for (const segment of path) {
    current = current?.[segment];
  }
  return (current as JsonSchema) ?? schema;
}

export function mergeAllOf(schema: JsonSchema, root: JsonSchema): JsonSchema {
  if (!schema.allOf) return schema;
  const merged: JsonSchema = { ...schema };
  delete merged.allOf;
  for (const sub of schema.allOf) {
    const resolved = resolveRef(sub, root);
    if (resolved.properties) {
      merged.properties = { ...merged.properties, ...resolved.properties };
    }
    if (resolved.required) {
      merged.required = [...(merged.required ?? []), ...resolved.required];
    }
    if (resolved.additionalProperties !== undefined) {
      merged.additionalProperties = resolved.additionalProperties;
    }
    if (resolved.description && !merged.description) {
      merged.description = resolved.description;
    }
    if (resolved.type && !merged.type) {
      merged.type = resolved.type;
    }
    if (resolved.oneOf && !merged.oneOf) {
      merged.oneOf = resolved.oneOf;
    }
  }
  return merged;
}

/** Fully resolve a schema: follow $ref, merge allOf, unwrap nullable anyOf */
export function fullyResolve(schema: JsonSchema, root: JsonSchema): JsonSchema {
  let s = resolveRef(schema, root);
  s = mergeAllOf(s, root);

  // Unwrap nullable anyOf: [{ $ref: ... }, { type: "null" }]
  if (s.anyOf && s.anyOf.length === 2) {
    const nonNull = s.anyOf.find(
      (v) => !(v.type === "null" || (Array.isArray(v.type) && v.type.includes("null")))
    );
    if (nonNull) {
      const resolved = fullyResolve(nonNull, root);
      return {
        ...resolved,
        description: s.description ?? resolved.description,
        default: s.default ?? resolved.default,
        nullable: true,
      };
    }
  }

  return s;
}

/** Detect tagged union: oneOf where each variant has properties.type with enum/const */
export function isTaggedUnion(schema: JsonSchema): boolean {
  if (!schema.oneOf || schema.oneOf.length < 2) return false;
  return schema.oneOf.every((variant) => {
    const typeField = variant.properties?.type;
    return typeField && (typeField.enum || typeField.const !== undefined);
  });
}

/** Extract variant tag values and their sub-schemas from a tagged union */
export function getVariants(
  schema: JsonSchema
): { tag: string; schema: JsonSchema; description?: string }[] {
  if (!schema.oneOf) return [];
  return schema.oneOf.map((variant) => {
    const typeField = variant.properties?.type;
    const tag = typeField?.const ?? typeField?.enum?.[0] ?? "unknown";
    return { tag, schema: variant, description: variant.description };
  });
}

const SENSITIVE_PATTERNS = /(?:api_key|password|secret|token|credential|private_key)/i;

export function isSensitiveField(name: string): boolean {
  return SENSITIVE_PATTERNS.test(name);
}

/** Get the primary scalar type from a schema, handling nullable type arrays */
export function getPrimaryType(schema: JsonSchema): string | undefined {
  if (typeof schema.type === "string") return schema.type;
  if (Array.isArray(schema.type)) {
    return schema.type.find((t) => t !== "null") ?? schema.type[0];
  }
  return undefined;
}

/** Check if a schema represents a map (object with additionalProperties but few/no fixed properties) */
export function isMapSchema(schema: JsonSchema): boolean {
  if (getPrimaryType(schema) !== "object") return false;
  if (!schema.additionalProperties || schema.additionalProperties === true) return false;
  const propCount = Object.keys(schema.properties ?? {}).length;
  return propCount === 0;
}

/** Human-readable label from a snake_case field name */
export function fieldLabel(name: string): string {
  return name
    .replace(/_/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase())
    .replace(/\bUrl\b/g, "URL")
    .replace(/\bApi\b/g, "API")
    .replace(/\bIp\b/g, "IP")
    .replace(/\bTtl\b/g, "TTL")
    .replace(/\bSso\b/g, "SSO")
    .replace(/\bMs\b/g, "ms")
    .replace(/\bSecs\b/g, "Seconds")
    .replace(/\bMb\b/g, "MB")
    .replace(/\bOcr\b/g, "OCR")
    .replace(/\bDpi\b/g, "DPI")
    .replace(/\bSsl\b/g, "SSL")
    .replace(/\bTls\b/g, "TLS")
    .replace(/\bHsts\b/g, "HSTS")
    .replace(/\bOtlp\b/g, "OTLP")
    .replace(/\bCel\b/g, "CEL")
    .replace(/\bRbac\b/g, "RBAC")
    .replace(/\bOidc\b/g, "OIDC")
    .replace(/\bSaml\b/g, "SAML")
    .replace(/\bDlq\b/g, "DLQ");
}

/** Top-level config sections in the desired tab order */
export const CONFIG_SECTIONS = [
  "server",
  "database",
  "cache",
  "providers",
  "auth",
  "features",
  "limits",
  "observability",
  "pricing",
  "ui",
  "docs",
  "secrets",
  "storage",
  "retention",
] as const;

export const SECTION_LABELS: Record<string, string> = {
  server: "Server",
  database: "Database",
  cache: "Cache",
  providers: "Providers",
  auth: "Auth",
  features: "Features",
  limits: "Limits",
  observability: "Observability",
  pricing: "Pricing",
  ui: "UI",
  docs: "Docs",
  secrets: "Secrets",
  storage: "Storage",
  retention: "Retention",
};
