import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ToastProvider } from "@/components/Toast/Toast";
import { ImageEditingPanel } from "./ImageEditingPanel";

const meta = {
  title: "Studio/ImageEditingPanel",
  component: ImageEditingPanel,
  parameters: {
    layout: "fullscreen",
  },
  decorators: [
    (Story) => {
      const queryClient = new QueryClient({
        defaultOptions: { queries: { retry: false } },
      });
      return (
        <QueryClientProvider client={queryClient}>
          <ToastProvider>
            <div className="h-[600px]">
              <Story />
            </div>
          </ToastProvider>
        </QueryClientProvider>
      );
    },
  ],
} satisfies Meta<typeof ImageEditingPanel>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
