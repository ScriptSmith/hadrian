import { PROVIDER_COLORS } from "@/pages/providers/shared";

/** Model capabilities from the catalog */
export interface ModelCapabilities {
  vision: boolean;
  reasoning: boolean;
  tool_call: boolean;
  structured_output: boolean;
  temperature: boolean;
}

/** Model modalities from the catalog */
export interface ModelModalities {
  input: string[];
  output: string[];
}

/** Catalog pricing in dollars per 1M tokens */
export interface CatalogPricing {
  input: number;
  output: number;
  reasoning?: number;
  cache_read?: number;
  cache_write?: number;
}

export interface ModelInfo {
  id: string;
  object?: string;
  created?: number;
  owned_by?: string;
  /** Whether this model comes from a built-in ("static") or user-added ("dynamic") provider */
  source?: "static" | "dynamic";
  /** For dynamic models, the provider name within the scope */
  provider_name?: string;
  context_length?: number;
  max_output_tokens?: number;
  pricing?: {
    prompt?: string;
    completion?: string;
  };
  description?: string;
  /** Model capabilities from the catalog */
  capabilities?: ModelCapabilities;
  /** Model modalities from the catalog */
  modalities?: ModelModalities;
  /** Catalog pricing in dollars per 1M tokens */
  catalog_pricing?: CatalogPricing;
  /** Model family (e.g., "claude-opus", "gpt-4") */
  family?: string;
  /** Whether the model has open weights */
  open_weights?: boolean;
  /** Supported tasks / API endpoints (e.g., "chat", "image_generation", "tts") */
  tasks?: string[];
  /** Knowledge cutoff date (ISO format or YYYY-MM) */
  knowledge_cutoff?: string;
  /** Release date (ISO format or YYYY-MM-DD) */
  release_date?: string;
  /** Supported image sizes for image generation models */
  image_sizes?: string[];
  /** Supported image quality options for image generation models */
  image_qualities?: string[];
  /** Maximum images per generation request */
  max_images?: number;
  /** Available voices for TTS models */
  voices?: string[];
  [key: string]: unknown;
}

/** Scope of a dynamic provider model, derived from the scoped model ID format */
export type DynamicScope = "user" | "org" | "project";

/** Determine the dynamic scope from a model ID, or undefined for static models */
export function getDynamicScope(modelId: string): DynamicScope | undefined {
  const parts = modelId.split("/");
  if (parts[0] === ":user") return "user";
  if (parts[0] === ":org" && parts.length >= 4) {
    if (parts[2] === ":user") return "user";
    if (parts[2] === ":project") return "project";
    return "org";
  }
  return undefined;
}

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

export function getProviderFromId(modelId: string): string {
  const parts = modelId.split("/");
  if (parts[0] === ":org" && parts.length >= 4) {
    if ((parts[2] === ":user" || parts[2] === ":project") && parts.length >= 6) {
      return parts[4];
    }
    return parts[2];
  }
  if (parts[0] === ":user" && parts.length >= 4) {
    return parts[2];
  }
  return parts.length > 1 ? parts[0] : "";
}

/** Map of provider IDs to display labels */
const PROVIDER_LABELS: Record<string, string> = {
  anthropic: "Anthropic",
  openai: "OpenAI",
  google: "Google",
  meta: "Meta",
  mistral: "Mistral",
  cohere: "Cohere",
  deepseek: "DeepSeek",
  qwen: "Qwen",
  openrouter: "OpenRouter",
  test: "Test",
};

export function getProviderInfo(
  provider: string,
  source?: "static" | "dynamic"
): { color: string; label: string } {
  const normalizedProvider = provider.toLowerCase();
  const entry = PROVIDER_COLORS[normalizedProvider];

  // Known providers keep their own colors regardless of source
  if (entry) {
    return {
      color: entry.badge,
      label: PROVIDER_LABELS[normalizedProvider] ?? provider,
    };
  }

  // Dynamic (user-added) providers get a distinct teal/emerald style
  if (source === "dynamic") {
    return {
      color: "bg-emerald-500/10 text-emerald-800 dark:text-emerald-400",
      label: provider,
    };
  }

  return {
    color: "bg-gray-500/10 text-gray-700 dark:text-gray-400",
    label: provider,
  };
}

export function formatContextLength(length?: number): string {
  if (!length) return "";
  if (length >= 1000000) return `${(length / 1000000).toFixed(1)}M`;
  if (length >= 1000) return `${Math.round(length / 1000)}k`;
  return String(length);
}

export function formatPrice(pricePerToken: string | number | undefined): string {
  if (!pricePerToken) return "";
  const price = typeof pricePerToken === "string" ? parseFloat(pricePerToken) : pricePerToken;
  if (isNaN(price)) return "";
  const perMillion = price * 1_000_000;
  if (perMillion >= 1) return perMillion.toFixed(2);
  if (perMillion >= 0.01) return perMillion.toFixed(3);
  return perMillion.toFixed(4);
}

/**
 * Get the model type for display.
 * Uses catalog capabilities when available, falls back to name heuristics.
 */
export function getModelType(
  modelId: string,
  capabilities?: ModelCapabilities,
  modalities?: ModelModalities
): { label: string; icon: "sparkles" | "cpu" } {
  // If we have catalog data, use it
  if (capabilities || modalities) {
    // Check for multimodal (vision)
    if (capabilities?.vision || modalities?.input?.includes("image")) {
      return { label: "Multimodal", icon: "sparkles" };
    }
  }

  // Fall back to name-based heuristics
  const name = modelId.toLowerCase();
  if (name.includes("vision") || name.includes("4o") || name.includes("gemini"))
    return { label: "Multimodal", icon: "sparkles" };
  if (name.includes("embed") || name.includes("embedding"))
    return { label: "Embedding", icon: "cpu" };
  if (name.includes("instruct")) return { label: "Instruct", icon: "cpu" };
  if (name.includes("code") || name.includes("coder")) return { label: "Code", icon: "cpu" };
  return { label: "Chat", icon: "sparkles" };
}

/**
 * Format catalog pricing for display (dollars per 1M tokens).
 */
export function formatCatalogPricing(dollars: number | undefined): string {
  if (dollars === undefined || dollars === null) return "";
  if (dollars === 0) return "Free";
  if (dollars < 0.01) return `$${dollars.toFixed(4)}`;
  if (dollars < 1) return `$${dollars.toFixed(3)}`;
  return `$${dollars.toFixed(2)}`;
}

/** Available capability filter values */
export type CapabilityFilter =
  | "all"
  | "reasoning"
  | "tool_call"
  | "vision"
  | "structured_output"
  | "open_weights";

/** Check if a model matches a capability filter */
export function matchesCapabilityFilter(model: ModelInfo, filter: CapabilityFilter): boolean {
  if (filter === "all") return true;

  const capabilities = model.capabilities;
  const modalities = model.modalities;

  switch (filter) {
    case "reasoning":
      return capabilities?.reasoning === true;
    case "tool_call":
      return capabilities?.tool_call === true;
    case "vision":
      return capabilities?.vision === true || modalities?.input?.includes("image") === true;
    case "structured_output":
      return capabilities?.structured_output === true;
    case "open_weights":
      return model.open_weights === true;
    default:
      return true;
  }
}

/**
 * Format max output tokens for display.
 */
export function formatMaxOutputTokens(tokens?: number): string {
  if (!tokens) return "";
  if (tokens >= 1000000) return `${(tokens / 1000000).toFixed(1)}M`;
  if (tokens >= 1000) return `${Math.round(tokens / 1000)}k`;
  return String(tokens);
}

/**
 * Format a date string for display (YYYY-MM or YYYY-MM-DD → Month Year or Month Day, Year).
 */
export function formatDate(dateStr?: string): string {
  if (!dateStr) return "";
  try {
    // Handle YYYY-MM format
    if (/^\d{4}-\d{2}$/.test(dateStr)) {
      const [year, month] = dateStr.split("-");
      const date = new Date(Number(year), Number(month) - 1);
      return date.toLocaleDateString("en-US", { month: "short", year: "numeric" });
    }
    // Handle full date formats
    const date = new Date(dateStr);
    if (isNaN(date.getTime())) return dateStr;
    return date.toLocaleDateString("en-US", { month: "short", year: "numeric" });
  } catch {
    return dateStr;
  }
}

/**
 * Check if a model has extra details worth showing in the expandable panel.
 */
export function hasExtraDetails(model: ModelInfo): boolean {
  return !!(
    model.max_output_tokens ||
    model.family ||
    model.knowledge_cutoff ||
    model.release_date ||
    model.description
  );
}
