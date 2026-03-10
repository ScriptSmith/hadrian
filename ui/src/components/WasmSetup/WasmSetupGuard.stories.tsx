import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { WasmSetupGuard } from "./WasmSetupGuard";
import { ApiClientProvider } from "@/api/ApiClientProvider";
import { ConfigProvider } from "@/config/ConfigProvider";
import { AuthProvider } from "@/auth";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});

const meta: Meta<typeof WasmSetupGuard> = {
  title: "Components/WasmSetupGuard",
  component: WasmSetupGuard,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <ConfigProvider>
          <AuthProvider>
            <ApiClientProvider>
              <Story />
            </ApiClientProvider>
          </AuthProvider>
        </ConfigProvider>
      </QueryClientProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => (
    <WasmSetupGuard>
      <div className="p-8 text-center">
        <p className="text-lg font-medium">App Content</p>
        <p className="text-sm text-muted-foreground">In non-WASM mode, children render directly.</p>
      </div>
    </WasmSetupGuard>
  ),
};
