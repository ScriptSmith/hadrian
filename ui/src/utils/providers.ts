/** Provider configuration for styling model badges */

export interface ProviderStyle {
  name: string;
  color: string;
  bgColor: string;
  borderColor: string;
}

const providerStyles: Record<string, ProviderStyle> = {
  anthropic: {
    name: "Anthropic",
    // Toned down orange/amber for better light mode compatibility
    color: "text-amber-700 dark:text-amber-400",
    bgColor: "bg-amber-50 dark:bg-amber-950/40",
    borderColor: "border-amber-200/70 dark:border-amber-800/50",
  },
  openai: {
    name: "OpenAI",
    color: "text-emerald-700 dark:text-emerald-400",
    bgColor: "bg-emerald-50 dark:bg-emerald-950/40",
    borderColor: "border-emerald-200/70 dark:border-emerald-800/50",
  },
  google: {
    name: "Google",
    color: "text-blue-700 dark:text-blue-400",
    bgColor: "bg-blue-50 dark:bg-blue-950/40",
    borderColor: "border-blue-200/70 dark:border-blue-800/50",
  },
  amazon: {
    name: "Amazon",
    color: "text-orange-700 dark:text-orange-400",
    bgColor: "bg-orange-50 dark:bg-orange-950/40",
    borderColor: "border-orange-200/70 dark:border-orange-800/50",
  },
  mistral: {
    name: "Mistral",
    color: "text-violet-700 dark:text-violet-400",
    bgColor: "bg-violet-50 dark:bg-violet-950/40",
    borderColor: "border-violet-200/70 dark:border-violet-800/50",
  },
  meta: {
    name: "Meta",
    color: "text-sky-700 dark:text-sky-400",
    bgColor: "bg-sky-50 dark:bg-sky-950/40",
    borderColor: "border-sky-200/70 dark:border-sky-800/50",
  },
  cohere: {
    name: "Cohere",
    color: "text-pink-700 dark:text-pink-400",
    bgColor: "bg-pink-50 dark:bg-pink-950/40",
    borderColor: "border-pink-200/70 dark:border-pink-800/50",
  },
  groq: {
    name: "Groq",
    color: "text-cyan-700 dark:text-cyan-400",
    bgColor: "bg-cyan-50 dark:bg-cyan-950/40",
    borderColor: "border-cyan-200/70 dark:border-cyan-800/50",
  },
  deepseek: {
    name: "DeepSeek",
    color: "text-indigo-700 dark:text-indigo-400",
    bgColor: "bg-indigo-50 dark:bg-indigo-950/40",
    borderColor: "border-indigo-200/70 dark:border-indigo-800/50",
  },
  xai: {
    name: "xAI",
    color: "text-slate-700 dark:text-slate-300",
    bgColor: "bg-slate-50 dark:bg-slate-800/40",
    borderColor: "border-slate-200/70 dark:border-slate-700/50",
  },
  default: {
    name: "Unknown",
    color: "text-gray-600 dark:text-gray-400",
    bgColor: "bg-gray-50 dark:bg-gray-800/40",
    borderColor: "border-gray-200/70 dark:border-gray-700/50",
  },
};

/** Detect provider from model ID */
export function detectProvider(modelId: string): string {
  const lower = modelId.toLowerCase();

  if (lower.includes("claude") || lower.includes("anthropic")) return "anthropic";
  if (lower.includes("gpt") || lower.includes("o1") || lower.includes("openai")) return "openai";
  if (lower.includes("gemini") || lower.includes("google")) return "google";
  if (lower.includes("nova") || lower.includes("titan") || lower.includes("amazon"))
    return "amazon";
  if (lower.includes("mistral") || lower.includes("mixtral") || lower.includes("codestral"))
    return "mistral";
  if (lower.includes("llama") || lower.includes("meta")) return "meta";
  if (lower.includes("command") || lower.includes("cohere")) return "cohere";
  if (lower.includes("groq")) return "groq";
  if (lower.includes("deepseek")) return "deepseek";
  if (lower.includes("grok") || lower.includes("xai")) return "xai";

  return "default";
}

/** Get styling for a provider */
export function getProviderStyle(provider: string): ProviderStyle {
  return providerStyles[provider] || providerStyles.default;
}

/** Get styling for a model by its ID */
export function getModelStyle(modelId: string): ProviderStyle {
  const provider = detectProvider(modelId);
  return getProviderStyle(provider);
}
