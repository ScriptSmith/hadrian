import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";
import { ToolsMenu } from "./ToolsMenu";

const meta: Meta<typeof ToolsMenu> = {
  title: "Components/ChatInput/ToolsMenu",
  component: ToolsMenu,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div className="p-8">
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof ToolsMenu>;

function ToolsMenuWithState(props: Partial<React.ComponentProps<typeof ToolsMenu>>) {
  const [enabledTools, setEnabledTools] = useState<string[]>(props.enabledTools ?? []);
  return (
    <ToolsMenu
      enabledTools={enabledTools}
      onEnabledToolsChange={setEnabledTools}
      vectorStoreIds={props.vectorStoreIds}
      disabled={props.disabled}
    />
  );
}

export const Default: Story = {
  render: () => <ToolsMenuWithState />,
};

export const WithCodeInterpreterEnabled: Story = {
  render: () => <ToolsMenuWithState enabledTools={["code_interpreter"]} />,
};

export const WithVectorStores: Story = {
  render: () => <ToolsMenuWithState vectorStoreIds={["vs_123"]} enabledTools={["file_search"]} />,
};

export const WithMultipleTools: Story = {
  render: () => (
    <ToolsMenuWithState
      vectorStoreIds={["vs_123"]}
      enabledTools={["file_search", "code_interpreter"]}
    />
  ),
};

export const Disabled: Story = {
  render: () => <ToolsMenuWithState disabled />,
};
