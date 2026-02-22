import type { Meta, StoryObj } from "@storybook/react";

import { ApiKeyCreatedModal } from "./ApiKeyCreatedModal";

const meta: Meta<typeof ApiKeyCreatedModal> = {
  title: "Admin/ApiKeyCreatedModal",
  component: ApiKeyCreatedModal,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof ApiKeyCreatedModal>;

export const Default: Story = {
  args: {
    apiKey: "hdr_sk_live_abc123def456ghi789jkl012mno345pqr678stu901vwx234yz",
    onClose: () => console.log("Close"),
  },
};

export const ShortKey: Story = {
  args: {
    apiKey: "hdr_sk_test_short123",
    onClose: () => console.log("Close"),
  },
};

export const LongKey: Story = {
  args: {
    apiKey:
      "hdr_sk_live_verylongapikeythatmightwrapacrossmultiplelinesintheuiwhendisplayedtousersforcopyingpurposes12345678901234567890",
    onClose: () => console.log("Close"),
  },
};

export const Closed: Story = {
  args: {
    apiKey: null,
    onClose: () => console.log("Close"),
  },
};
