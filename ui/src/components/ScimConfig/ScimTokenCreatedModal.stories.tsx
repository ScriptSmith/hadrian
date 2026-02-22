import type { Meta, StoryObj } from "@storybook/react";

import { ScimTokenCreatedModal } from "./ScimTokenCreatedModal";

const meta: Meta<typeof ScimTokenCreatedModal> = {
  title: "Admin/ScimTokenCreatedModal",
  component: ScimTokenCreatedModal,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof ScimTokenCreatedModal>;

export const Default: Story = {
  args: {
    token: "scim_abc123def456ghi789jkl012mno345pqr678stu901vwx234yz",
    onClose: () => console.log("Close"),
  },
};

export const ShortToken: Story = {
  args: {
    token: "scim_short123abc",
    onClose: () => console.log("Close"),
  },
};

export const LongToken: Story = {
  args: {
    token:
      "scim_verylongtokenthatmightwrapacrossmultiplelinesintheuiwhendisplayedtousersforcopyingpurposes12345678901234567890abcdefghijklmnop",
    onClose: () => console.log("Close"),
  },
};

export const Closed: Story = {
  args: {
    token: null,
    onClose: () => console.log("Close"),
  },
};
