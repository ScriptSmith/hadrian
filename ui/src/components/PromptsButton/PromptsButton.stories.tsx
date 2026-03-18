import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { Meta, StoryObj } from "@storybook/react";
import { MemoryRouter } from "react-router-dom";
import { TemplatesButton } from "./PromptsButton";
import { AuthProvider } from "@/auth";
import { ConfigProvider } from "@/config/ConfigProvider";
import { ToastProvider } from "../Toast/Toast";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
    },
  },
});

const meta: Meta<typeof TemplatesButton> = {
  title: "Chat/TemplatesButton",
  component: TemplatesButton,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/chat"]}>
          <ConfigProvider>
            <AuthProvider>
              <ToastProvider>
                <div className="flex items-center gap-2 p-4">
                  <Story />
                </div>
              </ToastProvider>
            </AuthProvider>
          </ConfigProvider>
        </MemoryRouter>
      </QueryClientProvider>
    ),
  ],
  args: {
    onApplyTemplate: (content: string) => console.log("Apply template:", content),
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Disabled: Story = {
  args: {
    disabled: true,
  },
};
