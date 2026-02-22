import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fn } from "storybook/test";
import { HttpResponse, http } from "msw";
import { CelExpressionInput } from "./CelExpressionInput";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false, staleTime: Infinity },
  },
});

const meta: Meta<typeof CelExpressionInput> = {
  title: "Admin/CelExpressionInput",
  component: CelExpressionInput,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <Story />
      </QueryClientProvider>
    ),
  ],
  args: {
    value: "",
    onChange: fn(),
    disabled: false,
  },
  parameters: {
    msw: {
      handlers: [
        http.post("/admin/v1/rbac-policies/validate", async ({ request }) => {
          const body = (await request.json()) as { condition: string };
          const condition = body.condition;
          // Simulate validation - invalid if contains "invalid"
          if (condition.includes("invalid")) {
            return HttpResponse.json({ valid: false, error: "Invalid CEL syntax at position 0" });
          }
          // Valid expression
          return HttpResponse.json({ valid: true, error: null });
        }),
      ],
    },
  },
};

export default meta;
type Story = StoryObj<typeof CelExpressionInput>;

export const Empty: Story = {
  args: {
    value: "",
  },
};

export const ValidExpression: Story = {
  args: {
    value: "'admin' in subject.roles",
  },
};

export const InvalidExpression: Story = {
  args: {
    value: "invalid expression here",
  },
};

export const WithExternalError: Story = {
  args: {
    value: "'admin' in subject.roles",
    error: "Condition is required",
  },
};

export const Disabled: Story = {
  args: {
    value: "'admin' in subject.roles",
    disabled: true,
  },
};

export const ComplexExpression: Story = {
  args: {
    value: `subject.email.endsWith('@acme.com') &&
  ('admin' in subject.roles || context.action == 'read') &&
  context.org_id in subject.org_ids`,
  },
};
