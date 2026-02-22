/**
 * Extract a short display name from a full model ID.
 * e.g., "anthropic/claude-3-opus" -> "Claude 3 Opus"
 * e.g., "gpt-4-turbo" -> "GPT-4 Turbo"
 */
export function getShortModelName(model: string): string {
  if (!model) return "";
  // Remove scope/provider prefix - take the last segment
  const parts = model.split("/");
  const name = parts[parts.length - 1];
  // Note: for scoped IDs like :org/.../provider/model, last segment is still the model name

  // Convert common patterns to readable names
  return name
    .replace(/^claude-/, "Claude ")
    .replace(/^gpt-/, "GPT-")
    .replace(/^gemini-/, "Gemini ")
    .replace(/-/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase())
    .trim();
}
