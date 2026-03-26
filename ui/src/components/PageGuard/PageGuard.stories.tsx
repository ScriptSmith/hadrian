import type { Meta, StoryObj } from "@storybook/react";
import { http, HttpResponse } from "msw";
import { MemoryRouter } from "react-router-dom";

import { ConfigProvider } from "@/config/ConfigProvider";
import { defaultConfig } from "@/config/defaults";
import type { UiConfig } from "@/config/types";
import { PageGuard } from "./PageGuard";

const enabledConfig: UiConfig = {
  ...defaultConfig,
  pages: {
    ...defaultConfig.pages,
    providers: { status: "enabled" },
  },
};

const noticeConfig: UiConfig = {
  ...defaultConfig,
  pages: {
    ...defaultConfig.pages,
    providers: {
      status: "notice",
      notice_message: "Provider management is temporarily disabled for maintenance.",
    },
  },
};

const disabledConfig: UiConfig = {
  ...defaultConfig,
  pages: {
    ...defaultConfig.pages,
    providers: { status: "disabled" },
  },
};

function createHandlers(config: UiConfig) {
  return [http.get("*/admin/v1/ui/config", () => HttpResponse.json(config))];
}

const meta: Meta<typeof PageGuard> = {
  title: "Components/PageGuard",
  component: PageGuard,
  decorators: [
    (Story) => (
      <MemoryRouter>
        <ConfigProvider>
          <Story />
        </ConfigProvider>
      </MemoryRouter>
    ),
  ],
  parameters: {
    layout: "centered",
    msw: { handlers: createHandlers(enabledConfig) },
  },
};

export default meta;
type Story = StoryObj<typeof PageGuard>;

export const Enabled: Story = {
  args: {
    pageKey: "providers",
    pageTitle: "Providers",
    children: <div className="p-8 text-center">Page content is visible</div>,
  },
  parameters: {
    msw: { handlers: createHandlers(enabledConfig) },
  },
};

export const Notice: Story = {
  args: {
    pageKey: "providers",
    pageTitle: "Providers",
    children: <div className="p-8 text-center">This should not be visible</div>,
  },
  parameters: {
    msw: { handlers: createHandlers(noticeConfig) },
  },
};

export const Disabled: Story = {
  args: {
    pageKey: "providers",
    pageTitle: "Providers",
    children: <div className="p-8 text-center">This should not be visible</div>,
  },
  parameters: {
    msw: { handlers: createHandlers(disabledConfig) },
  },
};
