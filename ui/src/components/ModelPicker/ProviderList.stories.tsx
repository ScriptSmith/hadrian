import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";

import { ProviderList } from "./ProviderList";
import type { ProviderFilter, ProviderInfo } from "./ProviderList";

const meta = {
  title: "Components/ModelPicker/ProviderList",
  component: ProviderList,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div className="w-[200px] border rounded-lg bg-popover">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ProviderList>;

export default meta;
type Story = StoryObj<typeof meta>;

const sampleProviders: ProviderInfo[] = [
  { id: "openai", label: "OpenAI", color: "bg-green-500", modelCount: 12 },
  { id: "anthropic", label: "Anthropic", color: "bg-orange-500", modelCount: 8 },
  { id: "google", label: "Google", color: "bg-blue-500", modelCount: 6 },
  { id: "mistral", label: "Mistral", color: "bg-cyan-500", modelCount: 4 },
  { id: "meta", label: "Meta", color: "bg-purple-500", modelCount: 3 },
  { id: "cohere", label: "Cohere", color: "bg-pink-500", modelCount: 2 },
];

function InteractiveProviderList({
  providers,
  totalModelCount,
  favoriteCount,
  selectedCount,
  initialProvider = "all",
  horizontal = false,
}: {
  providers: ProviderInfo[];
  totalModelCount: number;
  favoriteCount: number;
  selectedCount: number;
  initialProvider?: ProviderFilter;
  horizontal?: boolean;
}) {
  const [selectedProvider, setSelectedProvider] = useState<ProviderFilter>(initialProvider);

  return (
    <ProviderList
      providers={providers}
      selectedProvider={selectedProvider}
      onSelectProvider={setSelectedProvider}
      totalModelCount={totalModelCount}
      favoriteCount={favoriteCount}
      selectedCount={selectedCount}
      horizontal={horizontal}
    />
  );
}

export const Default: Story = {
  render: () => (
    <InteractiveProviderList
      providers={sampleProviders}
      totalModelCount={35}
      favoriteCount={5}
      selectedCount={0}
    />
  ),
};

export const WithSelection: Story = {
  render: () => (
    <InteractiveProviderList
      providers={sampleProviders}
      totalModelCount={35}
      favoriteCount={5}
      selectedCount={3}
    />
  ),
};

export const ProviderSelected: Story = {
  render: () => (
    <InteractiveProviderList
      providers={sampleProviders}
      totalModelCount={35}
      favoriteCount={5}
      selectedCount={2}
      initialProvider="anthropic"
    />
  ),
};

export const FavoritesSelected: Story = {
  render: () => (
    <InteractiveProviderList
      providers={sampleProviders}
      totalModelCount={35}
      favoriteCount={5}
      selectedCount={0}
      initialProvider="favorites"
    />
  ),
};

export const NoFavorites: Story = {
  render: () => (
    <InteractiveProviderList
      providers={sampleProviders}
      totalModelCount={35}
      favoriteCount={0}
      selectedCount={0}
    />
  ),
};

export const ManyProviders: Story = {
  render: () => (
    <InteractiveProviderList
      providers={[
        ...sampleProviders,
        { id: "deepseek", label: "DeepSeek", color: "bg-indigo-500", modelCount: 5 },
        { id: "qwen", label: "Qwen", color: "bg-teal-500", modelCount: 4 },
        { id: "openrouter", label: "OpenRouter", color: "bg-violet-500", modelCount: 20 },
      ]}
      totalModelCount={64}
      favoriteCount={8}
      selectedCount={1}
    />
  ),
};

export const SingleProvider: Story = {
  render: () => (
    <InteractiveProviderList
      providers={[{ id: "openai", label: "OpenAI", color: "bg-green-500", modelCount: 5 }]}
      totalModelCount={5}
      favoriteCount={2}
      selectedCount={0}
    />
  ),
};

export const WithDynamicProviders: Story = {
  render: () => (
    <InteractiveProviderList
      providers={[
        ...sampleProviders,
        {
          id: "org:my-org-openrouter",
          label: "my-org-openrouter",
          color: "bg-emerald-500",
          modelCount: 15,
          isDynamic: true,
          dynamicScope: "org",
        },
        {
          id: "user:my-openai",
          label: "my-openai",
          color: "bg-emerald-500",
          modelCount: 3,
          isDynamic: true,
          dynamicScope: "user",
        },
        {
          id: "user:my-anthropic",
          label: "my-anthropic",
          color: "bg-emerald-500",
          modelCount: 2,
          isDynamic: true,
          dynamicScope: "user",
        },
      ]}
      totalModelCount={55}
      favoriteCount={5}
      selectedCount={0}
    />
  ),
};

export const HorizontalMobile: Story = {
  decorators: [
    (Story) => (
      <div className="w-[360px] border rounded-lg bg-popover overflow-hidden">
        <Story />
      </div>
    ),
  ],
  render: () => (
    <InteractiveProviderList
      providers={sampleProviders}
      totalModelCount={35}
      favoriteCount={5}
      selectedCount={2}
      horizontal={true}
    />
  ),
};
