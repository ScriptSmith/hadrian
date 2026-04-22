import type { Skill } from "@/api/generated/types.gen";

/**
 * Process-wide cache for skill metadata, indexed by skill name.
 *
 * The chat's `Skill` tool executor needs two things at runtime:
 * 1. A name → id lookup so it can call `skillGet({ id })`.
 * 2. A "have I already fetched this skill's full files?" cache so we don't
 *    refetch on every tool call within the same conversation.
 *
 * Both live here as simple module-scoped maps. `useUserSkills` populates
 * `skillsByName` whenever its result changes (see `useSkillCacheSync`),
 * and the executor populates `fullSkillsById` on first fetch.
 *
 * This is intentionally a vanilla JS singleton (not a Zustand store): tool
 * executors run outside React's render tree and need synchronous lookup.
 */
const skillsByName: Map<string, Skill> = new Map();
const fullSkillsById: Map<string, Skill> = new Map();

export function setSkillCatalog(skills: Skill[]): void {
  skillsByName.clear();
  for (const s of skills) {
    skillsByName.set(s.name, s);
  }
}

export function getSkillByName(name: string): Skill | undefined {
  return skillsByName.get(name);
}

export function getFullSkill(id: string): Skill | undefined {
  return fullSkillsById.get(id);
}

export function setFullSkill(skill: Skill): void {
  fullSkillsById.set(skill.id, skill);
}

export function clearSkillCache(): void {
  skillsByName.clear();
  fullSkillsById.clear();
}
