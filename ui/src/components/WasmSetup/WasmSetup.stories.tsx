import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { WasmSetup } from "./WasmSetup";
import { Button } from "../Button/Button";
import { ApiClientProvider } from "@/api/ApiClientProvider";
import { ConfigProvider } from "@/config/ConfigProvider";
import { AuthProvider } from "@/auth";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});

const meta: Meta<typeof WasmSetup> = {
  title: "Components/WasmSetup",
  component: WasmSetup,
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

function InteractiveStory() {
  const [open, setOpen] = useState(true);
  return (
    <>
      <Button onClick={() => setOpen(true)}>Open Setup Wizard</Button>
      <WasmSetup open={open} onComplete={() => setOpen(false)} />
    </>
  );
}

export const Default: Story = {
  render: () => <InteractiveStory />,
};
