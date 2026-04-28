import { parseSkillMd } from "./parseFrontmatter";
import type { DiscoveredSkill } from "./githubImport";

import { formatApiError } from "@/utils/formatApiError";
const utf8Encoder = new TextEncoder();

/**
 * Byte length the server will see for `content`. The server enforces its
 * size limit using Rust's `String::len()` which counts UTF-8 bytes;
 * `String.prototype.length` in JS counts UTF-16 code units, so we'd
 * under- or over-report any non-ASCII content.
 */
function utf8ByteLength(content: string): number {
  return utf8Encoder.encode(content).length;
}

/**
 * Group a `FileList` (typically from `<input webkitdirectory>` or a
 * directory drop event) by the directory that contains a `SKILL.md`, then
 * read every file under each group as text. Binary files are rejected —
 * v1 skills are text-only by backend contract.
 *
 * `File.webkitRelativePath` gives "<top-dir>/<rel/to/skill>/file.ext".
 * The directory containing the SKILL.md becomes the skill directory; its
 * siblings (scripts/, references/, assets/) are collected alongside.
 */
export async function walkFilesForSkills(files: File[]): Promise<DiscoveredSkill[]> {
  if (files.length === 0) return [];

  // Index every file by its full relative path for fast parent lookup.
  const byPath = new Map<string, File>();
  for (const f of files) {
    const rel = f.webkitRelativePath || f.name;
    byPath.set(rel, f);
  }

  // Every path ending in "/SKILL.md" marks a skill directory.
  const skillDirs: string[] = [];
  for (const rel of byPath.keys()) {
    if (rel.endsWith("/SKILL.md") || rel === "SKILL.md") {
      const dir = rel === "SKILL.md" ? "" : rel.slice(0, -"/SKILL.md".length);
      skillDirs.push(dir);
    }
  }
  // Skills don't nest: drop any dir that's contained inside another skill.
  const filteredDirs = skillDirs.filter(
    (d) => !skillDirs.some((other) => other !== d && d.startsWith(other + "/"))
  );

  const out: DiscoveredSkill[] = [];
  for (const dir of filteredDirs) {
    const prefix = dir ? dir + "/" : "";
    const skillFiles: { path: string; content: string }[] = [];
    let error: string | undefined;

    for (const [rel, file] of byPath) {
      if (!rel.startsWith(prefix)) continue;
      if (dir && rel === dir) continue; // ignore the dir entry itself

      const sub = rel.slice(prefix.length);
      if (!sub) continue;

      let text: string;
      try {
        text = await file.text();
      } catch (err) {
        error = `Failed to read ${sub}: ${err instanceof Error ? err.message : formatApiError(err)}`;
        continue;
      }

      // Reject binary-looking content. A null byte is the simplest reliable
      // heuristic; we deliberately scan for the control char.
      if (text.includes("\u0000")) {
        error = `Binary file not supported: ${sub}`;
        continue;
      }

      skillFiles.push({ path: sub, content: text });
    }

    const main = skillFiles.find((f) => f.path === "SKILL.md");
    const fallbackName = (dir.split("/").pop() || "skill").toLowerCase();
    const total_bytes = skillFiles.reduce((sum, f) => sum + utf8ByteLength(f.content), 0);

    if (!main) {
      out.push({
        skillDir: dir,
        name: fallbackName,
        description: "",
        files: skillFiles,
        total_bytes,
        frontmatter: { extra: {} },
        error: error ?? "SKILL.md missing",
      });
      continue;
    }

    const parsed = parseSkillMd(main.content);
    out.push({
      skillDir: dir,
      name: parsed.frontmatter.name ?? fallbackName,
      description: parsed.frontmatter.description ?? "",
      files: skillFiles,
      total_bytes,
      frontmatter: parsed.frontmatter,
      error,
    });
  }

  return out;
}
