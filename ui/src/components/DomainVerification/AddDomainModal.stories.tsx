import type { Meta, StoryObj } from "@storybook/react";
import { fn } from "storybook/test";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { http, HttpResponse, delay } from "msw";
import { AddDomainModal } from "./AddDomainModal";
import type { DomainVerification } from "@/api/generated/types.gen";
import { ToastProvider } from "@/components/Toast/Toast";

const mockCreatedDomain: DomainVerification = {
  id: "domain-new",
  org_sso_config_id: "config-1",
  domain: "example.com",
  verification_token: "new-token-123",
  status: "pending",
  verification_attempts: 0,
  created_at: new Date().toISOString(),
  updated_at: new Date().toISOString(),
};

const createQueryClient = () =>
  new QueryClient({
    defaultOptions: {
      queries: { retry: false, staleTime: Infinity },
    },
  });

const meta: Meta<typeof AddDomainModal> = {
  title: "Admin/AddDomainModal",
  component: AddDomainModal,

  decorators: [
    (Story) => (
      <QueryClientProvider client={createQueryClient()}>
        <ToastProvider>
          <div style={{ minHeight: "400px" }}>
            <Story />
          </div>
        </ToastProvider>
      </QueryClientProvider>
    ),
  ],
  args: {
    open: true,
    onClose: fn(),
    orgSlug: "acme-corp",
  },
};

export default meta;
type Story = StoryObj<typeof AddDomainModal>;

export const Default: Story = {
  parameters: {
    msw: {
      handlers: [
        http.post("*/admin/v1/organizations/acme-corp/sso-config/domains", () => {
          return HttpResponse.json(mockCreatedDomain, { status: 201 });
        }),
      ],
    },
  },
};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.post("*/admin/v1/organizations/acme-corp/sso-config/domains", async () => {
          await delay("infinite");
          return HttpResponse.json(mockCreatedDomain, { status: 201 });
        }),
      ],
    },
  },
};

export const Error: Story = {
  parameters: {
    msw: {
      handlers: [
        http.post("*/admin/v1/organizations/acme-corp/sso-config/domains", () => {
          return HttpResponse.json(
            { error: "Domain already exists for this organization" },
            { status: 409 }
          );
        }),
      ],
    },
  },
};

export const PublicDomainBlocked: Story = {
  parameters: {
    msw: {
      handlers: [
        http.post("*/admin/v1/organizations/acme-corp/sso-config/domains", () => {
          return HttpResponse.json(
            { error: "Public email domains (gmail.com, hotmail.com, etc.) cannot be verified" },
            { status: 400 }
          );
        }),
      ],
    },
  },
};
