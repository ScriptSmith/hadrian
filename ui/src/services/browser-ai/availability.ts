import type { LanguageModelAvailability, LanguageModelGlobal } from "./types";

export const BROWSER_AI_PROVIDER = "browser";
export const BROWSER_AI_PREFIX = `${BROWSER_AI_PROVIDER}/`;

/**
 * Best-guess identifier for the on-device model behind the Prompt API. The
 * spec exposes no model name, so we infer one from the user agent. Chrome and
 * Brave ship Gemini Nano; Edge announced Phi-4 Mini for its on-device stack.
 * Anything else falls back to a generic id that still lets the routing layer
 * recognise the model via the `browser/` prefix.
 */
export function detectBrowserAiModel(): { id: string; modelName: string; vendor: string } {
  const ua = typeof navigator !== "undefined" ? (navigator.userAgent ?? "") : "";
  // The doubled `browser-` prefix on the model name surfaces as
  // "Browser <Name>" in the model picker after the provider segment is
  // stripped (formatModelName splits on hyphens). Without it the picker
  // would show just "Gemini Nano" with no Browser-AI cue.
  if (/\bEdg\//.test(ua)) {
    return {
      id: `${BROWSER_AI_PREFIX}browser-phi-4-mini`,
      modelName: "browser-phi-4-mini",
      vendor: "Edge",
    };
  }
  if (/\b(?:Chrome|Chromium|Brave)\//.test(ua) || ua.includes(" Brave/")) {
    return {
      id: `${BROWSER_AI_PREFIX}browser-gemini-nano`,
      modelName: "browser-gemini-nano",
      vendor: "Chromium",
    };
  }
  return {
    id: `${BROWSER_AI_PREFIX}browser-on-device`,
    modelName: "browser-on-device",
    vendor: "Browser",
  };
}

export function getLanguageModel(): LanguageModelGlobal | null {
  if (typeof globalThis === "undefined") return null;
  const lm = (globalThis as unknown as { LanguageModel?: LanguageModelGlobal }).LanguageModel;
  return lm ?? null;
}

export function isLanguageModelSupported(): boolean {
  return getLanguageModel() !== null;
}

export async function getAvailability(): Promise<LanguageModelAvailability> {
  const lm = getLanguageModel();
  if (!lm) return "unavailable";
  try {
    return await lm.availability();
  } catch {
    return "unavailable";
  }
}
