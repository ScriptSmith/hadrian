import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";
import { Button } from "@/components/Button/Button";

import { SelfServiceApiKeyFormModal } from "./SelfServiceApiKeyFormModal";

const meta: Meta<typeof SelfServiceApiKeyFormModal> = {
  title: "Admin/SelfServiceApiKeyFormModal",
  component: SelfServiceApiKeyFormModal,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof SelfServiceApiKeyFormModal>;

export const Default: Story = {
  args: {
    isOpen: true,
    onClose: () => console.log("Close"),
    onSubmit: (data) => console.log("Submit", data),
  },
};

export const Loading: Story = {
  args: {
    isOpen: true,
    onClose: () => console.log("Close"),
    onSubmit: (data) => console.log("Submit", data),
    isLoading: true,
  },
};

function InteractiveDemo() {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <div className="p-4">
      <Button onClick={() => setIsOpen(true)}>Create API Key</Button>
      <SelfServiceApiKeyFormModal
        isOpen={isOpen}
        onClose={() => setIsOpen(false)}
        onSubmit={(data) => {
          console.log("Submitted:", data);
          setIsOpen(false);
        }}
      />
    </div>
  );
}

export const Interactive: Story = {
  render: () => <InteractiveDemo />,
};
