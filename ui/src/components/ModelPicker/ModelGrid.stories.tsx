import type { Meta, StoryObj } from "@storybook/react";
import { useState, useMemo } from "react";

import { ModelGrid } from "./ModelGrid";
import type { ModelInfo } from "./model-utils";

const meta = {
  title: "Components/ModelPicker/ModelGrid",
  component: ModelGrid,
  parameters: {
    layout: "padded",
  },
  decorators: [
    (Story) => (
      <div className="max-w-3xl border rounded-lg bg-popover">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ModelGrid>;

export default meta;
type Story = StoryObj<typeof meta>;

const sampleModels: ModelInfo[] = [
  {
    id: "openai/gpt-4o",
    owned_by: "openai",
    context_length: 128000,
    max_output_tokens: 16384,
    pricing: { prompt: "0.000005", completion: "0.000015" },
    capabilities: {
      vision: true,
      reasoning: false,
      tool_call: true,
      structured_output: true,
      temperature: true,
    },
    catalog_pricing: { input: 5.0, output: 15.0 },
    family: "gpt-4o",
    knowledge_cutoff: "2024-10",
    release_date: "2024-05-13",
    description: "GPT-4o is OpenAI's most advanced multimodal model with vision capabilities.",
  },
  {
    id: "openai/gpt-4o-mini",
    owned_by: "openai",
    context_length: 128000,
    max_output_tokens: 16384,
    pricing: { prompt: "0.00000015", completion: "0.0000006" },
    capabilities: {
      vision: true,
      reasoning: false,
      tool_call: true,
      structured_output: true,
      temperature: true,
    },
    catalog_pricing: { input: 0.15, output: 0.6 },
    family: "gpt-4o",
    knowledge_cutoff: "2024-10",
  },
  {
    id: "anthropic/claude-3-5-sonnet-20241022",
    owned_by: "anthropic",
    context_length: 200000,
    max_output_tokens: 8192,
    pricing: { prompt: "0.000003", completion: "0.000015" },
    capabilities: {
      vision: true,
      reasoning: false,
      tool_call: true,
      structured_output: false,
      temperature: true,
    },
    catalog_pricing: { input: 3.0, output: 15.0 },
    family: "claude-3.5-sonnet",
    knowledge_cutoff: "2024-04",
    release_date: "2024-10-22",
    description: "Claude 3.5 Sonnet combines high performance with fast response times.",
  },
  {
    id: "anthropic/claude-3-5-haiku-20241022",
    owned_by: "anthropic",
    context_length: 200000,
    max_output_tokens: 8192,
    pricing: { prompt: "0.0000008", completion: "0.000004" },
    capabilities: {
      vision: true,
      reasoning: false,
      tool_call: true,
      structured_output: false,
      temperature: true,
    },
    catalog_pricing: { input: 0.8, output: 4.0 },
    family: "claude-3.5-haiku",
    knowledge_cutoff: "2024-04",
  },
  {
    id: "google/gemini-2.0-flash-exp",
    owned_by: "google",
    context_length: 1000000,
    max_output_tokens: 8192,
    pricing: { prompt: "0", completion: "0" },
    capabilities: {
      vision: true,
      reasoning: false,
      tool_call: true,
      structured_output: true,
      temperature: true,
    },
    catalog_pricing: { input: 0, output: 0 },
    family: "gemini-2.0",
    release_date: "2024-12",
  },
  {
    id: "google/gemini-1.5-pro",
    owned_by: "google",
    context_length: 2000000,
    max_output_tokens: 8192,
    pricing: { prompt: "0.00000125", completion: "0.000005" },
    capabilities: {
      vision: true,
      reasoning: false,
      tool_call: true,
      structured_output: true,
      temperature: true,
    },
    catalog_pricing: { input: 1.25, output: 5.0 },
    family: "gemini-1.5",
    knowledge_cutoff: "2024-05",
  },
  {
    id: "mistral/mistral-large-latest",
    owned_by: "mistral",
    context_length: 128000,
    pricing: { prompt: "0.000002", completion: "0.000006" },
    capabilities: {
      vision: false,
      reasoning: false,
      tool_call: true,
      structured_output: true,
      temperature: true,
    },
    catalog_pricing: { input: 2.0, output: 6.0 },
    family: "mistral-large",
  },
  {
    id: "deepseek/deepseek-chat",
    owned_by: "deepseek",
    context_length: 64000,
    max_output_tokens: 8192,
    pricing: { prompt: "0.00000014", completion: "0.00000028" },
    capabilities: {
      vision: false,
      reasoning: true,
      tool_call: true,
      structured_output: false,
      temperature: true,
    },
    catalog_pricing: { input: 0.14, output: 0.28 },
    open_weights: true,
    family: "deepseek-v3",
    knowledge_cutoff: "2024-07",
    description: "DeepSeek-V3 with MoE architecture and advanced reasoning capabilities.",
  },
  {
    id: "meta/llama-3.3-70b-instruct",
    owned_by: "meta",
    context_length: 128000,
    max_output_tokens: 4096,
    capabilities: {
      vision: false,
      reasoning: false,
      tool_call: true,
      structured_output: false,
      temperature: true,
    },
    open_weights: true,
    family: "llama-3.3",
    knowledge_cutoff: "2024-03",
    release_date: "2024-12-06",
  },
  {
    id: "cohere/command-r-plus",
    owned_by: "cohere",
    context_length: 128000,
    pricing: { prompt: "0.0000025", completion: "0.00001" },
    capabilities: {
      vision: false,
      reasoning: false,
      tool_call: true,
      structured_output: false,
      temperature: true,
    },
    catalog_pricing: { input: 2.5, output: 10.0 },
    family: "command-r",
  },
  {
    id: "openai/text-embedding-3-large",
    owned_by: "openai",
    context_length: 8191,
    pricing: { prompt: "0.00000013", completion: "0" },
  },
  {
    id: "qwen/qwen-2.5-coder-32b-instruct",
    owned_by: "qwen",
    context_length: 32000,
    capabilities: {
      vision: false,
      reasoning: false,
      tool_call: false,
      structured_output: false,
      temperature: true,
    },
    open_weights: true,
    family: "qwen-2.5",
    description:
      "Specialized coding model from the Qwen team with strong code generation abilities.",
  },
  {
    id: "openai/o1-preview",
    owned_by: "openai",
    context_length: 128000,
    max_output_tokens: 32768,
    pricing: { prompt: "0.000015", completion: "0.00006" },
    capabilities: {
      vision: false,
      reasoning: true,
      tool_call: false,
      structured_output: false,
      temperature: false,
    },
    catalog_pricing: { input: 15.0, output: 60.0 },
    family: "o1",
    knowledge_cutoff: "2024-10",
    release_date: "2024-09-12",
    description: "OpenAI's reasoning model that thinks step-by-step before responding.",
  },
];

function InteractiveModelGrid({
  models,
  initialSelected = [],
  initialFavorites = [],
  initialDefaults = [],
  maxModels = 10,
}: {
  models: ModelInfo[];
  initialSelected?: string[];
  initialFavorites?: string[];
  initialDefaults?: string[];
  maxModels?: number;
}) {
  const [selectedModels, setSelectedModels] = useState<string[]>(initialSelected);
  const [favoriteModels, setFavoriteModels] = useState<string[]>(initialFavorites);
  const [defaultModels, setDefaultModels] = useState<string[]>(initialDefaults);

  // Create Sets for O(1) lookups
  const selectedSet = useMemo(() => new Set(selectedModels), [selectedModels]);
  const favoriteSet = useMemo(() => new Set(favoriteModels), [favoriteModels]);
  const defaultSet = useMemo(() => new Set(defaultModels), [defaultModels]);

  const handleToggleModel = (modelId: string) => {
    setSelectedModels((prev) =>
      prev.includes(modelId) ? prev.filter((m) => m !== modelId) : [...prev, modelId]
    );
  };

  const handleToggleFavorite = (modelId: string) => {
    setFavoriteModels((prev) =>
      prev.includes(modelId) ? prev.filter((m) => m !== modelId) : [...prev, modelId]
    );
  };

  const handleToggleDefault = (modelId: string) => {
    setDefaultModels((prev) =>
      prev.includes(modelId) ? prev.filter((m) => m !== modelId) : [...prev, modelId]
    );
  };

  return (
    <ModelGrid
      models={models}
      selectedSet={selectedSet}
      favoriteSet={favoriteSet}
      defaultSet={defaultSet}
      maxModels={maxModels}
      focusedIndex={-1}
      shouldScrollToFocused={false}
      onToggleModel={handleToggleModel}
      onToggleFavorite={handleToggleFavorite}
      onToggleDefault={handleToggleDefault}
      onShowDetails={() => {}}
    />
  );
}

export const Default: Story = {
  render: () => <InteractiveModelGrid models={sampleModels} />,
};

export const WithSelections: Story = {
  render: () => (
    <InteractiveModelGrid
      models={sampleModels}
      initialSelected={["openai/gpt-4o", "anthropic/claude-3-5-sonnet-20241022"]}
    />
  ),
};

export const WithFavorites: Story = {
  render: () => (
    <InteractiveModelGrid
      models={sampleModels}
      initialFavorites={[
        "openai/gpt-4o",
        "anthropic/claude-3-5-sonnet-20241022",
        "google/gemini-2.0-flash-exp",
      ]}
    />
  ),
};

export const WithDefaults: Story = {
  render: () => (
    <InteractiveModelGrid
      models={sampleModels}
      initialDefaults={["openai/gpt-4o-mini", "anthropic/claude-3-5-haiku-20241022"]}
    />
  ),
};

export const MaxModelsReached: Story = {
  render: () => (
    <InteractiveModelGrid
      models={sampleModels}
      initialSelected={["openai/gpt-4o", "anthropic/claude-3-5-sonnet-20241022"]}
      maxModels={2}
    />
  ),
};

export const FewModels: Story = {
  render: () => <InteractiveModelGrid models={sampleModels.slice(0, 3)} />,
};

export const ManyModels: Story = {
  render: () => (
    <InteractiveModelGrid
      models={[
        ...sampleModels,
        { id: "openai/gpt-4-turbo", owned_by: "openai", context_length: 128000 },
        { id: "openai/gpt-3.5-turbo", owned_by: "openai", context_length: 16000 },
        { id: "anthropic/claude-3-opus-20240229", owned_by: "anthropic", context_length: 200000 },
        { id: "mistral/mistral-small-latest", owned_by: "mistral", context_length: 32000 },
      ]}
    />
  ),
};

export const Empty: Story = {
  render: () => <InteractiveModelGrid models={[]} />,
};

// Generate a large dataset for virtualization testing
function generateLargeModelList(count: number): ModelInfo[] {
  const providers = [
    "openai",
    "anthropic",
    "google",
    "mistral",
    "meta",
    "cohere",
    "deepseek",
    "qwen",
    "amazon",
    "microsoft",
  ];
  const modelTypes = ["gpt", "claude", "gemini", "mistral", "llama", "command", "deepseek", "qwen"];

  return Array.from({ length: count }, (_, i) => {
    const provider = providers[i % providers.length];
    const modelType = modelTypes[i % modelTypes.length];
    const variant = Math.floor(i / providers.length);
    return {
      id: `${provider}/${modelType}-${variant}-model-${i}`,
      owned_by: provider,
      context_length: [4096, 8192, 16384, 32768, 128000, 200000][i % 6],
      pricing:
        i % 3 === 0
          ? { prompt: `0.0000${(i % 10) + 1}`, completion: `0.0000${(i % 10) + 2}` }
          : undefined,
    };
  });
}

const largeModelList = generateLargeModelList(900);

/**
 * Tests virtualization performance with 900 models.
 * Only visible rows should be in the DOM at any time.
 * Open DevTools and inspect the DOM to verify virtualization is working.
 */
export const LargeDataset: Story = {
  render: () => (
    <div className="h-[500px]">
      <InteractiveModelGrid models={largeModelList} initialFavorites={[largeModelList[0].id]} />
    </div>
  ),
};

/**
 * Interactive keyboard navigation component for large dataset story
 */
function KeyboardNavModelGrid() {
  const [focusedIndex, setFocusedIndex] = useState(0);
  const [selectedModels, setSelectedModels] = useState<string[]>([]);

  const selectedSet = useMemo(() => new Set(selectedModels), [selectedModels]);
  const favoriteSet = useMemo(() => new Set<string>(), []);
  const defaultSet = useMemo(() => new Set<string>(), []);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    const cols = 3;
    const total = largeModelList.length;
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        setFocusedIndex((prev) => Math.min(prev + cols, total - 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        setFocusedIndex((prev) => Math.max(prev - cols, 0));
        break;
      case "ArrowRight":
        e.preventDefault();
        setFocusedIndex((prev) => Math.min(prev + 1, total - 1));
        break;
      case "ArrowLeft":
        e.preventDefault();
        setFocusedIndex((prev) => Math.max(prev - 1, 0));
        break;
      case "Enter": {
        e.preventDefault();
        const modelId = largeModelList[focusedIndex].id;
        setSelectedModels((prev) =>
          prev.includes(modelId) ? prev.filter((m) => m !== modelId) : [...prev, modelId]
        );
        break;
      }
    }
  };

  return (
    // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex, jsx-a11y/no-static-element-interactions -- story demo wrapper for keyboard navigation testing
    <div className="h-[500px]" tabIndex={0} onKeyDown={handleKeyDown}>
      <div className="mb-2 text-sm text-muted-foreground">
        Use arrow keys to navigate, Enter to select. Focused: {focusedIndex}
      </div>
      <ModelGrid
        models={largeModelList}
        selectedSet={selectedSet}
        favoriteSet={favoriteSet}
        defaultSet={defaultSet}
        maxModels={10}
        focusedIndex={focusedIndex}
        shouldScrollToFocused={true}
        onToggleModel={(id) =>
          setSelectedModels((prev) =>
            prev.includes(id) ? prev.filter((m) => m !== id) : [...prev, id]
          )
        }
        onToggleFavorite={() => {}}
        onToggleDefault={() => {}}
        onShowDetails={() => {}}
      />
    </div>
  );
}

/**
 * Tests keyboard navigation with virtualization.
 * Use arrow keys to navigate - the focused row should scroll into view.
 */
export const LargeDatasetWithKeyboardNav: Story = {
  render: () => <KeyboardNavModelGrid />,
};
