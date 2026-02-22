import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";

import { AuthProvider } from "@/auth";
import { ConfigProvider } from "@/config/ConfigProvider";
import { UserMenu } from "./UserMenu";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
    },
  },
});

const meta: Meta<typeof UserMenu> = {
  title: "Components/UserMenu",
  component: UserMenu,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <MemoryRouter>
          <ConfigProvider>
            <AuthProvider>
              <div className="p-4 flex justify-end">
                <Story />
              </div>
            </AuthProvider>
          </ConfigProvider>
        </MemoryRouter>
      </QueryClientProvider>
    ),
  ],
  parameters: {
    layout: "fullscreen",
  },
};

export default meta;
type Story = StoryObj<typeof UserMenu>;

export const Default: Story = {};

export const WithClassName: Story = {
  args: {
    className: "ring-2 ring-primary",
  },
};
