import { skillGet } from "@/api/generated/sdk.gen";

import { getFullSkill, getSkillByName, setFullSkill } from "./skillCache";
import type { ParsedToolCall } from "./toolCallParser";
import type { ToolExecutionResult, ToolExecutor } from "./toolExecutors";

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KiB`;
  return `${(n / (1024 * 1024)).toFixed(2)} MiB`;
}

function manifestText(skill: { files: { path: string; byte_size: number }[] }): string {
  const others = skill.files.filter((f) => f.path !== "SKILL.md");
  if (others.length === 0) return "";
  const lines = [
    "",
    "---",
    "",
    'Additional files in this skill. Call Skill again with `{command: "<name>", file: "<path>"}` to read any:',
    ...others.map((f) => `- ${f.path} (${formatBytes(f.byte_size)})`),
  ];
  return lines.join("\n");
}

interface SkillToolArgs {
  command?: string;
  file?: string | null;
}

function parseArgs(raw: unknown): SkillToolArgs {
  if (raw === null || raw === undefined) return {};
  if (typeof raw === "string") {
    if (!raw.trim()) return {};
    try {
      return JSON.parse(raw) as SkillToolArgs;
    } catch {
      return {};
    }
  }
  return raw as SkillToolArgs;
}

/**
 * Executes the `Skill` function tool registered with the LLM. Two modes:
 *
 * - `{command: "<name>"}`
 *   Returns the skill's SKILL.md body plus a manifest listing every bundled
 *   file by path and size.
 *
 * - `{command: "<name>", file: "<relative-path>"}`
 *   Returns the content of a bundled file (scripts/, references/, assets/)
 *   referenced in the SKILL.md.
 *
 * Matches Claude Code's progressive-disclosure architecture: the first call
 * pulls the main instructions into context; file calls load referenced
 * resources on demand, not eagerly.
 */
export const skillExecutor: ToolExecutor = async (
  toolCall: ParsedToolCall
): Promise<ToolExecutionResult> => {
  const args = parseArgs(toolCall.arguments);
  const command = args.command?.trim();
  if (!command) {
    return {
      success: false,
      error: "Missing required argument `command` (the skill name).",
    };
  }

  const summary = getSkillByName(command);
  if (!summary) {
    return {
      success: true,
      output: `Skill "${command}" is not available. Check the Available skills list for exact names.`,
    };
  }

  // Fetch the full skill on first use; subsequent calls hit the cache.
  let skill = getFullSkill(summary.id);
  if (!skill) {
    try {
      const response = await skillGet({ path: { id: summary.id } });
      if (response.error || !response.data) {
        throw new Error(
          typeof response.error === "object" && response.error && "message" in response.error
            ? String((response.error as { message: unknown }).message)
            : "Failed to load skill"
        );
      }
      skill = response.data;
      setFullSkill(skill);
    } catch (err) {
      return {
        success: false,
        error: `Failed to load skill "${command}": ${err instanceof Error ? err.message : String(err)}`,
      };
    }
  }

  const filePath = args.file?.trim();
  if (filePath) {
    const file = skill.files?.find((f) => f.path === filePath);
    if (!file) {
      const available = (skill.files ?? [])
        .filter((f) => f.path !== "SKILL.md")
        .map((f) => `  - ${f.path}`)
        .join("\n");
      return {
        success: true,
        output:
          `File "${filePath}" not found in skill "${command}".` +
          (available ? `\n\nAvailable files:\n${available}` : ""),
      };
    }
    return { success: true, output: file.content };
  }

  const main = skill.files?.find((f) => f.path === "SKILL.md");
  if (!main) {
    return {
      success: false,
      error: `Skill "${command}" is missing its SKILL.md file.`,
    };
  }

  const manifest = skill.files ? manifestText({ files: skill.files }) : "";
  return { success: true, output: main.content + manifest };
};
