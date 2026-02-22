/**
 * Human-readable display names for known models.
 * Maps model identifiers (after stripping provider prefix) to friendly names.
 */
const MODEL_DISPLAY_NAMES: Record<string, string> = {
  // OpenAI models
  "gpt-4o": "GPT-4o",
  "gpt-4o-mini": "GPT-4o Mini",
  "gpt-4-turbo": "GPT-4 Turbo",
  "gpt-4-turbo-preview": "GPT-4 Turbo Preview",
  "gpt-4": "GPT-4",
  "gpt-4-32k": "GPT-4 32K",
  "gpt-3.5-turbo": "GPT-3.5 Turbo",
  "gpt-3.5-turbo-16k": "GPT-3.5 Turbo 16K",
  o1: "o1",
  "o1-mini": "o1 Mini",
  "o1-preview": "o1 Preview",
  o3: "o3",
  "o3-mini": "o3 Mini",
  "o4-mini": "o4 Mini",

  // Anthropic models
  "claude-3-5-sonnet-20241022": "Claude 3.5 Sonnet",
  "claude-3-5-sonnet-latest": "Claude 3.5 Sonnet",
  "claude-3-5-haiku-20241022": "Claude 3.5 Haiku",
  "claude-3-5-haiku-latest": "Claude 3.5 Haiku",
  "claude-sonnet-4-20250514": "Claude Sonnet 4",
  "claude-sonnet-4-0": "Claude Sonnet 4",
  "claude-opus-4-20250514": "Claude Opus 4",
  "claude-opus-4-0": "Claude Opus 4",
  "claude-3-opus-20240229": "Claude 3 Opus",
  "claude-3-opus-latest": "Claude 3 Opus",
  "claude-3-sonnet-20240229": "Claude 3 Sonnet",
  "claude-3-haiku-20240307": "Claude 3 Haiku",
  "claude-2.1": "Claude 2.1",
  "claude-2.0": "Claude 2",
  "claude-instant-1.2": "Claude Instant",

  // Google models
  "gemini-2.0-flash": "Gemini 2.0 Flash",
  "gemini-2.0-flash-exp": "Gemini 2.0 Flash Exp",
  "gemini-2.0-flash-thinking-exp": "Gemini 2.0 Flash Thinking",
  "gemini-1.5-pro": "Gemini 1.5 Pro",
  "gemini-1.5-pro-latest": "Gemini 1.5 Pro",
  "gemini-1.5-flash": "Gemini 1.5 Flash",
  "gemini-1.5-flash-latest": "Gemini 1.5 Flash",
  "gemini-pro": "Gemini Pro",
  "gemini-pro-vision": "Gemini Pro Vision",

  // Meta models
  "llama-3.3-70b-instruct": "Llama 3.3 70B",
  "llama-3.2-90b-vision-instruct": "Llama 3.2 90B Vision",
  "llama-3.2-11b-vision-instruct": "Llama 3.2 11B Vision",
  "llama-3.2-3b-instruct": "Llama 3.2 3B",
  "llama-3.2-1b-instruct": "Llama 3.2 1B",
  "llama-3.1-405b-instruct": "Llama 3.1 405B",
  "llama-3.1-70b-instruct": "Llama 3.1 70B",
  "llama-3.1-8b-instruct": "Llama 3.1 8B",
  "llama-3-70b-instruct": "Llama 3 70B",
  "llama-3-8b-instruct": "Llama 3 8B",

  // Mistral models
  "mistral-large-latest": "Mistral Large",
  "mistral-large-2411": "Mistral Large",
  "mistral-medium-latest": "Mistral Medium",
  "mistral-small-latest": "Mistral Small",
  "mixtral-8x7b-instruct": "Mixtral 8x7B",
  "mixtral-8x22b-instruct": "Mixtral 8x22B",
  "codestral-latest": "Codestral",
  "pixtral-large-latest": "Pixtral Large",
  "pixtral-12b-2409": "Pixtral 12B",

  // DeepSeek models
  "deepseek-chat": "DeepSeek Chat",
  "deepseek-coder": "DeepSeek Coder",
  "deepseek-reasoner": "DeepSeek Reasoner",

  // Qwen models
  "qwen-turbo": "Qwen Turbo",
  "qwen-plus": "Qwen Plus",
  "qwen-max": "Qwen Max",
  "qwq-32b-preview": "QwQ 32B",

  // Cohere models
  "command-r-plus": "Command R+",
  "command-r": "Command R",
  command: "Command",
  "command-light": "Command Light",
};

/**
 * Get a human-readable display name for a model.
 *
 * @param modelId - The full model ID (e.g., "openai/gpt-4o" or "gpt-4o")
 * @returns A human-readable name, or a cleaned-up version of the ID if not found
 */
export function getModelDisplayName(modelId: string): string {
  // Extract just the model name (stripping scope/provider prefix)
  const modelName = getModelName(modelId);

  // Check if we have a known display name
  if (MODEL_DISPLAY_NAMES[modelName]) {
    return MODEL_DISPLAY_NAMES[modelName];
  }

  // Try case-insensitive match
  const lowerName = modelName.toLowerCase();
  for (const [key, value] of Object.entries(MODEL_DISPLAY_NAMES)) {
    if (key.toLowerCase() === lowerName) {
      return value;
    }
  }

  // Fall back to formatting the model name nicely
  return formatModelName(modelName);
}

/**
 * Format a model name for display when no known name exists.
 * Handles common patterns like "model-name-v1.2" -> "Model Name v1.2"
 */
function formatModelName(name: string): string {
  return (
    name
      // Split on hyphens and underscores
      .split(/[-_]/)
      // Capitalize first letter of each word, preserve numbers/versions
      .map((part) => {
        // If it's a version-like string (v1, 1.0, etc), keep as-is
        if (/^v?\d/.test(part)) return part;
        // Capitalize first letter
        return part.charAt(0).toUpperCase() + part.slice(1);
      })
      .join(" ")
  );
}

/**
 * Get the provider name from a full model ID.
 * Handles scoped dynamic provider IDs like:
 *   :org/{ORG}/:user/{USER}/{PROVIDER}/{MODEL}
 *   :org/{ORG}/{PROVIDER}/{MODEL}
 *   :user/{USER}/{PROVIDER}/{MODEL}
 */
export function getProviderFromModelId(modelId: string): string {
  const parts = modelId.split("/");
  if (parts[0] === ":org" && parts.length >= 4) {
    // :org/{ORG}/:user/{USER}/{PROVIDER}/... or :org/{ORG}/:project/{PROJECT}/{PROVIDER}/...
    if ((parts[2] === ":user" || parts[2] === ":project") && parts.length >= 6) {
      return parts[4];
    }
    // :org/{ORG}/{PROVIDER}/...
    return parts[2];
  }
  if (parts[0] === ":user" && parts.length >= 4) {
    // :user/{USER}/{PROVIDER}/...
    return parts[2];
  }
  return parts.length > 1 ? parts[0] : "";
}

/**
 * Get the raw model name (without provider/scope prefix) from a full model ID.
 * Handles scoped dynamic provider IDs.
 */
export function getModelName(modelId: string): string {
  const parts = modelId.split("/");
  if (parts[0] === ":org" && parts.length >= 4) {
    if ((parts[2] === ":user" || parts[2] === ":project") && parts.length >= 6) {
      return parts.slice(5).join("/");
    }
    return parts.slice(3).join("/");
  }
  if (parts[0] === ":user" && parts.length >= 4) {
    return parts.slice(3).join("/");
  }
  return parts.length > 1 ? parts.slice(1).join("/") : parts[0];
}
