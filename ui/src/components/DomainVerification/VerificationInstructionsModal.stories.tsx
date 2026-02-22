import type { Meta, StoryObj } from "@storybook/react";
import { fn } from "storybook/test";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { http, HttpResponse, delay } from "msw";
import { VerificationInstructionsModal } from "./VerificationInstructionsModal";
import type {
  DomainVerification,
  DomainVerificationInstructions,
  VerifyDomainResponse,
} from "@/api/generated/types.gen";
import { ToastProvider } from "@/components/Toast/Toast";

const mockPendingDomain: DomainVerification = {
  id: "domain-1",
  org_sso_config_id: "config-1",
  domain: "acme.com",
  verification_token: "abc123xyz789def456",
  status: "pending",
  verification_attempts: 1,
  last_attempt_at: "2024-01-14T10:00:00Z",
  created_at: "2024-01-10T09:00:00Z",
  updated_at: "2024-01-14T10:00:00Z",
};

const mockVerifiedDomain: DomainVerification = {
  ...mockPendingDomain,
  status: "verified",
  verified_at: "2024-01-15T10:30:00Z",
  dns_txt_record: "hadrian-verify=abc123xyz789def456",
};

const mockFailedDomain: DomainVerification = {
  ...mockPendingDomain,
  status: "failed",
  verification_attempts: 3,
};

const mockInstructions: DomainVerificationInstructions = {
  domain: "acme.com",
  record_type: "TXT",
  record_host: "_hadrian-verify.acme.com",
  record_value: "hadrian-verify=abc123xyz789def456",
  instructions:
    "To verify ownership of acme.com, add a DNS TXT record:\n\nHost: _hadrian-verify.acme.com\nType: TXT\nValue: hadrian-verify=abc123xyz789def456",
};

const createQueryClient = () =>
  new QueryClient({
    defaultOptions: {
      queries: { retry: false, staleTime: Infinity },
    },
  });

const meta: Meta<typeof VerificationInstructionsModal> = {
  title: "Admin/VerificationInstructionsModal",
  component: VerificationInstructionsModal,

  decorators: [
    (Story) => (
      <QueryClientProvider client={createQueryClient()}>
        <ToastProvider>
          <div style={{ minHeight: "600px" }}>
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
    domainId: "domain-1",
  },
};

export default meta;
type Story = StoryObj<typeof VerificationInstructionsModal>;

export const Pending: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp/sso-config/domains/domain-1", () => {
          return HttpResponse.json(mockPendingDomain);
        }),
        http.get(
          "*/admin/v1/organizations/acme-corp/sso-config/domains/domain-1/instructions",
          () => {
            return HttpResponse.json(mockInstructions);
          }
        ),
        http.post("*/admin/v1/organizations/acme-corp/sso-config/domains/domain-1/verify", () => {
          const response: VerifyDomainResponse = {
            verified: false,
            message: "DNS TXT record not found. Please ensure the record has propagated.",
            verification: { ...mockPendingDomain, verification_attempts: 2 },
          };
          return HttpResponse.json(response);
        }),
      ],
    },
  },
};

export const Verified: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp/sso-config/domains/domain-1", () => {
          return HttpResponse.json(mockVerifiedDomain);
        }),
        http.get(
          "*/admin/v1/organizations/acme-corp/sso-config/domains/domain-1/instructions",
          () => {
            return HttpResponse.json(mockInstructions);
          }
        ),
      ],
    },
  },
};

export const Failed: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp/sso-config/domains/domain-1", () => {
          return HttpResponse.json(mockFailedDomain);
        }),
        http.get(
          "*/admin/v1/organizations/acme-corp/sso-config/domains/domain-1/instructions",
          () => {
            return HttpResponse.json(mockInstructions);
          }
        ),
        http.post("*/admin/v1/organizations/acme-corp/sso-config/domains/domain-1/verify", () => {
          const response: VerifyDomainResponse = {
            verified: false,
            message: "DNS TXT record not found after 3 attempts.",
            verification: { ...mockFailedDomain, verification_attempts: 4 },
          };
          return HttpResponse.json(response);
        }),
      ],
    },
  },
};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp/sso-config/domains/domain-1", async () => {
          await delay("infinite");
          return HttpResponse.json(mockPendingDomain);
        }),
        http.get(
          "*/admin/v1/organizations/acme-corp/sso-config/domains/domain-1/instructions",
          async () => {
            await delay("infinite");
            return HttpResponse.json(mockInstructions);
          }
        ),
      ],
    },
  },
};

export const VerifyingInProgress: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp/sso-config/domains/domain-1", () => {
          return HttpResponse.json(mockPendingDomain);
        }),
        http.get(
          "*/admin/v1/organizations/acme-corp/sso-config/domains/domain-1/instructions",
          () => {
            return HttpResponse.json(mockInstructions);
          }
        ),
        http.post(
          "*/admin/v1/organizations/acme-corp/sso-config/domains/domain-1/verify",
          async () => {
            await delay("infinite");
            return HttpResponse.json({} as VerifyDomainResponse);
          }
        ),
      ],
    },
  },
};

export const VerifySuccess: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp/sso-config/domains/domain-1", () => {
          return HttpResponse.json(mockPendingDomain);
        }),
        http.get(
          "*/admin/v1/organizations/acme-corp/sso-config/domains/domain-1/instructions",
          () => {
            return HttpResponse.json(mockInstructions);
          }
        ),
        http.post("*/admin/v1/organizations/acme-corp/sso-config/domains/domain-1/verify", () => {
          const response: VerifyDomainResponse = {
            verified: true,
            dns_record_found: "hadrian-verify=abc123xyz789def456",
            message: "Domain verified successfully!",
            verification: mockVerifiedDomain,
          };
          return HttpResponse.json(response);
        }),
      ],
    },
  },
};
