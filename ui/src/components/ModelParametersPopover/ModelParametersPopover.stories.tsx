import type { Meta, StoryObj } from "@storybook/react";
import { useState, useCallback } from "react";
import { ModelParametersPopover } from "./ModelParametersPopover";
import type { ModelParameters } from "../chat-types";

const meta: Meta<typeof ModelParametersPopover> = {
  title: "Chat/ModelParametersPopover",
  component: ModelParametersPopover,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

const defaultParams: ModelParameters = {
  temperature: 1.0,
  maxTokens: 4096,
  topP: 1.0,
  frequencyPenalty: 0,
  presencePenalty: 0,
};

function DefaultStory() {
  const [params, setParams] = useState<ModelParameters>(defaultParams);
  const handleChange = useCallback((newParams: ModelParameters) => {
    setParams(newParams);
  }, []);

  return (
    <div className="p-4">
      <ModelParametersPopover
        modelName="claude-3-opus"
        parameters={params}
        onParametersChange={handleChange}
      />
    </div>
  );
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function CustomSettingsStory() {
  const [params, setParams] = useState<ModelParameters>({
    temperature: 0.7,
    maxTokens: 2048,
    topP: 0.9,
    frequencyPenalty: 0.5,
    presencePenalty: 0.2,
  });
  const handleChange = useCallback((newParams: ModelParameters) => {
    setParams(newParams);
  }, []);

  return (
    <div className="p-4">
      <ModelParametersPopover
        modelName="gpt-4"
        parameters={params}
        onParametersChange={handleChange}
      />
    </div>
  );
}

export const CustomSettings: Story = {
  render: () => <CustomSettingsStory />,
};

function WithSystemPromptStory() {
  const [params, setParams] = useState<ModelParameters>({
    systemPrompt:
      "You are a code expert specializing in TypeScript. Always provide concise, well-typed examples.",
    temperature: 0.5,
  });
  const handleChange = useCallback((newParams: ModelParameters) => {
    setParams(newParams);
  }, []);

  return (
    <div className="p-4">
      <p className="text-sm text-muted-foreground mb-4 max-w-xs">
        This model has a custom system prompt set. Click the settings icon to see and edit it.
      </p>
      <ModelParametersPopover
        modelName="claude-3-opus"
        parameters={params}
        onParametersChange={handleChange}
      />
    </div>
  );
}

export const WithSystemPrompt: Story = {
  render: () => <WithSystemPromptStory />,
};

function WithReasoningStory() {
  const [params, setParams] = useState<ModelParameters>({
    reasoning: {
      enabled: true,
      effort: "high",
    },
  });
  const handleChange = useCallback((newParams: ModelParameters) => {
    setParams(newParams);
  }, []);

  return (
    <div className="p-4">
      <p className="text-sm text-muted-foreground mb-4 max-w-xs">
        This model has reasoning (extended thinking) enabled with high effort.
      </p>
      <ModelParametersPopover
        modelName="o1-preview"
        parameters={params}
        onParametersChange={handleChange}
      />
    </div>
  );
}

export const WithReasoning: Story = {
  render: () => <WithReasoningStory />,
};
