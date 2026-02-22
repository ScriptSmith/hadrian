import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";

import { NumberInput } from "./NumberInput";

const meta: Meta<typeof NumberInput> = {
  title: "Components/NumberInput",
  component: NumberInput,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div style={{ width: "200px" }}>
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof NumberInput>;

// Wrapper component to manage state for interactive stories
function NumberInputDemo(
  props: Omit<React.ComponentProps<typeof NumberInput>, "value" | "onChange">
) {
  const [value, setValue] = useState(props.min ?? 50);
  return <NumberInput {...props} value={value} onChange={setValue} />;
}

function NumberInputAtMin() {
  const [value, setValue] = useState(0);
  return <NumberInput value={value} onChange={setValue} min={0} max={10} label="At minimum" />;
}

function NumberInputAtMax() {
  const [value, setValue] = useState(10);
  return <NumberInput value={value} onChange={setValue} min={0} max={10} label="At maximum" />;
}

export const Default: Story = {
  render: () => <NumberInputDemo aria-label="Number input" />,
};

export const WithLabel: Story = {
  render: () => <NumberInputDemo label="Quantity" />,
};

export const WithMinMax: Story = {
  render: () => <NumberInputDemo min={0} max={100} label="Percentage (0-100)" />,
};

export const WithStep: Story = {
  render: () => <NumberInputDemo min={0} max={100} step={10} label="Step by 10" />,
};

export const WithoutButtons: Story = {
  render: () => <NumberInputDemo showButtons={false} label="No buttons" />,
};

export const Disabled: Story = {
  render: () => <NumberInputDemo disabled label="Disabled" />,
};

export const AtMinimum: Story = {
  render: () => <NumberInputAtMin />,
};

export const AtMaximum: Story = {
  render: () => <NumberInputAtMax />,
};

export const DecimalStep: Story = {
  render: () => <NumberInputDemo min={0} max={1} step={0.1} label="Decimal (0.1 step)" />,
};
