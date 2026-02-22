import type { Meta, StoryObj } from "@storybook/react";
import { ToastProvider } from "@/components/Toast/Toast";
import { ImageGenerationPanel } from "./ImageGenerationPanel";

const meta = {
  title: "Studio/ImageGenerationPanel",
  component: ImageGenerationPanel,
  parameters: {
    layout: "fullscreen",
  },
  decorators: [
    (Story) => (
      <ToastProvider>
        <div className="h-[600px]">
          <Story />
        </div>
      </ToastProvider>
    ),
  ],
} satisfies Meta<typeof ImageGenerationPanel>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
