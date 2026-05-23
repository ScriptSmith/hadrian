import type { Meta, StoryObj } from "@storybook/react";
import { MCPApprovalRequest } from "./MCPApprovalRequest";

const meta: Meta<typeof MCPApprovalRequest> = {
  title: "Chat/MCPApprovalRequest",
  component: MCPApprovalRequest,
  args: {
    onRespond: (approve: boolean) => console.log("respond", approve),
  },
};

export default meta;
type Story = StoryObj<typeof MCPApprovalRequest>;

export const Pending: Story = {
  args: {
    approval: {
      id: "mcpr_1",
      approvalRequestId: "mcpr_1",
      serverLabel: "github",
      toolName: "create_issue",
      argumentsJson: '{"repo":"acme/app","title":"Bug: crash on load"}',
      parsedArguments: { repo: "acme/app", title: "Bug: crash on load" },
    },
  },
};

export const Approved: Story = {
  args: {
    approval: {
      ...Pending.args!.approval!,
      resolved: "approved",
    },
  },
};

export const Denied: Story = {
  args: {
    approval: {
      ...Pending.args!.approval!,
      resolved: "denied",
    },
  },
};

export const Disabled: Story = {
  args: {
    ...Pending.args,
    disabled: true,
  },
};
