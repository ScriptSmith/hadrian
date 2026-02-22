import type { Meta, StoryObj } from "@storybook/react";
import { ExpandableCaption } from "./ExpandableCaption";

const meta = {
  title: "Studio/ExpandableCaption",
  component: ExpandableCaption,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div className="w-[280px] border rounded-lg p-3">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ExpandableCaption>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Short: Story = {
  args: {
    text: "A beautiful sunset over mountains.",
  },
};

export const Long: Story = {
  args: {
    text: "A breathtaking panoramic view of snow-capped mountain peaks illuminated by the warm golden light of a setting sun, with vibrant orange and pink hues painting the dramatic sky above while a serene alpine lake reflects the entire scene in its crystal-clear waters below, surrounded by ancient evergreen forests.",
  },
};

export const Expanded: Story = {
  args: {
    text: "A breathtaking panoramic view of snow-capped mountain peaks illuminated by the warm golden light of a setting sun, with vibrant orange and pink hues painting the dramatic sky above while a serene alpine lake reflects the entire scene in its crystal-clear waters below, surrounded by ancient evergreen forests.",
  },
};

export const ThreeLines: Story = {
  args: {
    text: "A breathtaking panoramic view of snow-capped mountain peaks illuminated by the warm golden light of a setting sun, with vibrant orange and pink hues painting the dramatic sky above while a serene alpine lake reflects the entire scene in its crystal-clear waters below, surrounded by ancient evergreen forests.",
    maxLines: 3,
  },
};
