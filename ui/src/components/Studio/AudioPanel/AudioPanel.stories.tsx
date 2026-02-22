import type { Meta, StoryObj } from "@storybook/react";
import { MemoryRouter } from "react-router-dom";
import { ToastProvider } from "@/components/Toast/Toast";
import { AudioPanel } from "./AudioPanel";

const meta = {
  title: "Studio/AudioPanel",
  component: AudioPanel,
  parameters: {
    layout: "fullscreen",
  },
} satisfies Meta<typeof AudioPanel>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Speak: Story = {
  decorators: [
    (Story) => (
      <MemoryRouter initialEntries={["/?tab=audio"]}>
        <ToastProvider>
          <div className="h-[700px]">
            <Story />
          </div>
        </ToastProvider>
      </MemoryRouter>
    ),
  ],
};

export const Transcribe: Story = {
  decorators: [
    (Story) => (
      <MemoryRouter initialEntries={["/?tab=audio&mode=transcribe"]}>
        <ToastProvider>
          <div className="h-[700px]">
            <Story />
          </div>
        </ToastProvider>
      </MemoryRouter>
    ),
  ],
};

export const Translate: Story = {
  decorators: [
    (Story) => (
      <MemoryRouter initialEntries={["/?tab=audio&mode=translate"]}>
        <ToastProvider>
          <div className="h-[700px]">
            <Story />
          </div>
        </ToastProvider>
      </MemoryRouter>
    ),
  ],
};
