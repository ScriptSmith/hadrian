import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";
import { Slider } from "./Slider";

const meta: Meta<typeof Slider> = {
  title: "UI/Slider",
  component: Slider,
  parameters: {
    layout: "centered",
  },

  decorators: [
    (Story) => (
      <div style={{ width: 300 }}>
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

function DefaultStory() {
  const [value, setValue] = useState(50);
  return <Slider value={value} onChange={setValue} aria-label="Slider" />;
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function WithLabelStory() {
  const [value, setValue] = useState(50);
  return <Slider value={value} onChange={setValue} label="Volume" showValue />;
}

export const WithLabel: Story = {
  render: () => <WithLabelStory />,
};

function CustomRangeStory() {
  const [value, setValue] = useState(0.5);
  return (
    <Slider
      value={value}
      onChange={setValue}
      min={0}
      max={1}
      step={0.1}
      label="Opacity"
      showValue
    />
  );
}

export const CustomRange: Story = {
  render: () => <CustomRangeStory />,
};

function TemperatureStory() {
  const [value, setValue] = useState(1.0);
  return (
    <Slider
      value={value}
      onChange={setValue}
      min={0}
      max={2}
      step={0.1}
      label="Temperature"
      showValue
    />
  );
}

export const Temperature: Story = {
  render: () => <TemperatureStory />,
};

export const Disabled: Story = {
  render: () => {
    return <Slider value={30} onChange={() => {}} disabled label="Disabled" showValue />;
  },
};
