import type { Meta, StoryObj } from "@storybook/react";
import { HadrianIcon } from "./HadrianIcon";

const meta: Meta<typeof HadrianIcon> = {
  title: "UI/HadrianIcon",
  component: HadrianIcon,
  parameters: {
    layout: "centered",
  },

  argTypes: {
    size: {
      control: "number",
    },
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  args: {
    size: 32,
  },
};

export const Small: Story = {
  args: {
    size: 16,
  },
};

export const Medium: Story = {
  args: {
    size: 24,
  },
};

export const Large: Story = {
  args: {
    size: 48,
  },
};

export const ExtraLarge: Story = {
  args: {
    size: 64,
  },
};

export const WithCustomClass: Story = {
  args: {
    size: 32,
    className: "text-primary",
  },
};
