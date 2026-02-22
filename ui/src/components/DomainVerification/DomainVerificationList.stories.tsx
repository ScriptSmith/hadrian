import type { Meta, StoryObj } from "@storybook/react";
import { fn } from "storybook/test";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { http, HttpResponse, delay } from "msw";
import { DomainVerificationList } from "./DomainVerificationList";
import type {
  DomainVerification,
  ListDomainVerificationsResponse,
} from "@/api/generated/types.gen";
import { ConfirmDialogProvider } from "@/components/ConfirmDialog/ConfirmDialog";
import { ToastProvider } from "@/components/Toast/Toast";

const mockDomains: DomainVerification[] = [
  {
    id: "domain-1",
    org_sso_config_id: "config-1",
    domain: "acme.com",
    verification_token: "abc123xyz",
    status: "verified",
    dns_txt_record: "hadrian-verify=abc123xyz",
    verification_attempts: 2,
    last_attempt_at: "2024-01-15T10:30:00Z",
    verified_at: "2024-01-15T10:30:00Z",
    created_at: "2024-01-10T09:00:00Z",
    updated_at: "2024-01-15T10:30:00Z",
  },
  {
    id: "domain-2",
    org_sso_config_id: "config-1",
    domain: "acme.org",
    verification_token: "def456uvw",
    status: "pending",
    verification_attempts: 0,
    created_at: "2024-01-12T14:00:00Z",
    updated_at: "2024-01-12T14:00:00Z",
  },
  {
    id: "domain-3",
    org_sso_config_id: "config-1",
    domain: "acme.io",
    verification_token: "ghi789rst",
    status: "failed",
    verification_attempts: 3,
    last_attempt_at: "2024-01-14T16:45:00Z",
    created_at: "2024-01-11T11:00:00Z",
    updated_at: "2024-01-14T16:45:00Z",
  },
];

const createQueryClient = () =>
  new QueryClient({
    defaultOptions: {
      queries: { retry: false, staleTime: Infinity },
    },
  });

const meta: Meta<typeof DomainVerificationList> = {
  title: "Admin/DomainVerificationList",
  component: DomainVerificationList,

  decorators: [
    (Story) => (
      <QueryClientProvider client={createQueryClient()}>
        <ToastProvider>
          <ConfirmDialogProvider>
            <div style={{ padding: "1rem", maxWidth: "800px" }}>
              <Story />
            </div>
          </ConfirmDialogProvider>
        </ToastProvider>
      </QueryClientProvider>
    ),
  ],
  args: {
    orgSlug: "acme-corp",
    onAddDomain: fn(),
    onViewInstructions: fn(),
  },
};

export default meta;
type Story = StoryObj<typeof DomainVerificationList>;

export const WithDomains: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp/sso-config/domains", () => {
          return HttpResponse.json({
            items: mockDomains,
            total: mockDomains.length,
          } satisfies ListDomainVerificationsResponse);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp/sso-config/domains", () => {
          return HttpResponse.json({
            items: [],
            total: 0,
          } satisfies ListDomainVerificationsResponse);
        }),
      ],
    },
  },
};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp/sso-config/domains", async () => {
          await delay("infinite");
          return HttpResponse.json({ items: [], total: 0 });
        }),
      ],
    },
  },
};

export const SingleVerified: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp/sso-config/domains", () => {
          return HttpResponse.json({
            items: [mockDomains[0]],
            total: 1,
          } satisfies ListDomainVerificationsResponse);
        }),
      ],
    },
  },
};

export const AllPending: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp/sso-config/domains", () => {
          const pendingDomains: DomainVerification[] = [
            { ...mockDomains[0], status: "pending", verified_at: undefined },
            { ...mockDomains[1] },
          ];
          return HttpResponse.json({
            items: pendingDomains,
            total: pendingDomains.length,
          } satisfies ListDomainVerificationsResponse);
        }),
      ],
    },
  },
};
