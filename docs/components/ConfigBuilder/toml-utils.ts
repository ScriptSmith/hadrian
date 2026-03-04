/* eslint-disable @typescript-eslint/no-explicit-any */
import { stringify } from "smol-toml";
import { type JsonSchema, fullyResolve, getPrimaryType, isTaggedUnion } from "./schema-utils";

/** Recursively remove null, undefined, empty strings, and empty objects/arrays */
export function stripEmpty(obj: any): any {
  if (obj === null || obj === undefined || obj === "") return undefined;
  if (Array.isArray(obj)) {
    const filtered = obj.map(stripEmpty).filter((v) => v !== undefined);
    return filtered.length > 0 ? filtered : undefined;
  }
  if (typeof obj === "object") {
    const result: Record<string, any> = {};
    let hasKeys = false;
    for (const [k, v] of Object.entries(obj)) {
      const stripped = stripEmpty(v);
      if (stripped !== undefined) {
        result[k] = stripped;
        hasKeys = true;
      }
    }
    return hasKeys ? result : undefined;
  }
  return obj;
}

/** Coerce string form values to their schema types (numbers, booleans) */
export function coerceTypes(obj: any, schema: JsonSchema | undefined, root: JsonSchema): any {
  if (obj === null || obj === undefined || !schema) return obj;

  const resolved = fullyResolve(schema, root);
  const type = getPrimaryType(resolved);

  if (typeof obj === "string") {
    if (type === "integer" || type === "number") {
      const n = Number(obj);
      return isNaN(n) ? obj : n;
    }
    if (type === "boolean") {
      return obj === "true";
    }
    return obj;
  }

  if (Array.isArray(obj)) {
    return obj.map((item) => coerceTypes(item, resolved.items, root));
  }

  if (typeof obj === "object") {
    const result: Record<string, any> = {};

    // For tagged unions, find the active variant schema
    if (isTaggedUnion(resolved) && obj.type) {
      const variant = resolved.oneOf?.find((v) => {
        const t = v.properties?.type;
        return t?.const === obj.type || t?.enum?.[0] === obj.type;
      });
      if (variant) {
        for (const [k, v] of Object.entries(obj)) {
          const propSchema = variant.properties?.[k];
          result[k] = coerceTypes(v, propSchema, root);
        }
        return result;
      }
    }

    for (const [k, v] of Object.entries(obj)) {
      const propSchema =
        resolved.properties?.[k] ??
        (typeof resolved.additionalProperties === "object"
          ? resolved.additionalProperties
          : undefined);
      result[k] = coerceTypes(v, propSchema, root);
    }
    return result;
  }

  return obj;
}

/** Generate TOML string from the config state */
export function generateToml(state: Record<string, any>, schema: JsonSchema): string {
  const stripped = stripEmpty(state);
  if (!stripped || Object.keys(stripped).length === 0) {
    return "# Empty configuration — select a section and fill in fields";
  }
  const coerced = coerceTypes(stripped, { type: "object", properties: schema.properties }, schema);
  try {
    return stringify(coerced);
  } catch {
    return "# Error: unable to generate valid TOML from current values";
  }
}
