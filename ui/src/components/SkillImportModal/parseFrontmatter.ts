/**
 * Tiny YAML frontmatter parser for SKILL.md imports.
 *
 * Covers just enough of the agentskills.io spec's subset of YAML to handle
 * the documented fields: string values (bare or quoted), arrays (inline
 * `[a, b]` or block `- a\n- b`), boolean values, and nested maps (only one
 * level deep — used for `metadata`).
 *
 * We deliberately avoid pulling in gray-matter / js-yaml for this: it's a
 * ~40-line surface area and the input format is tightly constrained.
 * Unknown/malformed fields are preserved verbatim in `extra` so the skill
 * still imports and round-trips.
 */

export interface ParsedFrontmatter {
  name?: string;
  description?: string;
  license?: string;
  compatibility?: string;
  argument_hint?: string;
  user_invocable?: boolean;
  disable_model_invocation?: boolean;
  /** Always normalized to `string[]` regardless of source syntax. */
  allowed_tools?: string[];
  metadata?: Record<string, unknown>;
  /** Raw key/value pairs not matched above, preserved as strings. */
  extra: Record<string, unknown>;
}

export interface ParsedSkillMd {
  frontmatter: ParsedFrontmatter;
  body: string;
}

function stripQuotes(raw: string): string {
  const trimmed = raw.trim();
  if (trimmed.startsWith('"') && trimmed.endsWith('"')) {
    // YAML double-quoted: unescape \" \\ \n \t (covers the common cases;
    // we don't need full YAML escape semantics for skill frontmatter).
    return trimmed
      .slice(1, -1)
      .replace(/\\"/g, '"')
      .replace(/\\n/g, "\n")
      .replace(/\\t/g, "\t")
      .replace(/\\\\/g, "\\");
  }
  if (trimmed.startsWith("'") && trimmed.endsWith("'")) {
    // YAML single-quoted: only `''` is meaningful (escapes a single quote).
    return trimmed.slice(1, -1).replace(/''/g, "'");
  }
  return trimmed;
}

function parseInlineList(raw: string): string[] {
  const inner = raw.trim();
  if (!inner.startsWith("[") || !inner.endsWith("]")) return [];
  return inner
    .slice(1, -1)
    .split(",")
    .map((s) => stripQuotes(s))
    .filter((s) => s.length > 0);
}

function parseAllowedTools(raw: string | string[]): string[] {
  if (Array.isArray(raw)) return raw;
  // Spec allows space-separated string: `Bash(git:*) Bash(jq:*) Read`
  return raw
    .split(/\s+/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

function parseBool(raw: string): boolean | undefined {
  const v = raw.trim().toLowerCase();
  if (v === "true" || v === "yes" || v === "on") return true;
  if (v === "false" || v === "no" || v === "off") return false;
  return undefined;
}

/**
 * Split the file at the `---` fences. Returns `{body: text}` if there's no
 * frontmatter block, leaving `frontmatter` empty.
 */
export function parseSkillMd(raw: string): ParsedSkillMd {
  // Normalize Windows line endings so the fence regex is stable.
  const text = raw.replace(/\r\n/g, "\n");

  if (!text.startsWith("---\n")) {
    return { frontmatter: { extra: {} }, body: text };
  }

  const endIdx = text.indexOf("\n---", 4);
  if (endIdx === -1) {
    return { frontmatter: { extra: {} }, body: text };
  }

  const yamlBlock = text.slice(4, endIdx);
  const bodyStart = text.indexOf("\n", endIdx + 4);
  const body = bodyStart === -1 ? "" : text.slice(bodyStart + 1);

  const lines = yamlBlock.split("\n");
  const out: ParsedFrontmatter = { extra: {} };
  const extra: Record<string, unknown> = {};

  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (!line.trim() || line.trim().startsWith("#")) {
      i++;
      continue;
    }

    // key: value   OR   key: (block list / map follows)
    const colonIdx = line.indexOf(":");
    if (colonIdx === -1) {
      i++;
      continue;
    }

    const key = line.slice(0, colonIdx).trim();
    const rest = line.slice(colonIdx + 1).trimEnd();
    const value = rest.trim();

    // Block-style value follows when the value is empty.
    if (value === "") {
      // Collect indented children.
      const children: string[] = [];
      i++;
      while (i < lines.length) {
        const next = lines[i];
        if (!next.startsWith("  ") && next.trim() !== "") break;
        if (next.trim()) children.push(next.slice(2));
        i++;
      }
      if (children.length > 0 && children[0].startsWith("- ")) {
        const items = children
          .filter((c) => c.startsWith("- "))
          .map((c) => stripQuotes(c.slice(2)));
        if (key === "allowed_tools" || key === "allowed-tools") {
          out.allowed_tools = items;
        } else {
          extra[key] = items;
        }
      } else {
        // Block map — one level deep, used for `metadata`.
        const map: Record<string, string> = {};
        for (const c of children) {
          const ci = c.indexOf(":");
          if (ci === -1) continue;
          map[c.slice(0, ci).trim()] = stripQuotes(c.slice(ci + 1));
        }
        if (key === "metadata") {
          out.metadata = map;
        } else {
          extra[key] = map;
        }
      }
      continue;
    }

    // Inline value.
    switch (key) {
      case "name":
        out.name = stripQuotes(value);
        break;
      case "description":
        out.description = stripQuotes(value);
        break;
      case "license":
        out.license = stripQuotes(value);
        break;
      case "compatibility":
        out.compatibility = stripQuotes(value);
        break;
      case "argument-hint":
      case "argument_hint":
        out.argument_hint = stripQuotes(value);
        break;
      case "user-invocable":
      case "user_invocable": {
        const b = parseBool(value);
        if (b !== undefined) out.user_invocable = b;
        break;
      }
      case "disable-model-invocation":
      case "disable_model_invocation": {
        const b = parseBool(value);
        if (b !== undefined) out.disable_model_invocation = b;
        break;
      }
      case "allowed-tools":
      case "allowed_tools":
        out.allowed_tools = value.startsWith("[")
          ? parseAllowedTools(parseInlineList(value))
          : parseAllowedTools(stripQuotes(value));
        break;
      default:
        extra[key] = stripQuotes(value);
    }
    i++;
  }

  out.extra = extra;
  return { frontmatter: out, body };
}
