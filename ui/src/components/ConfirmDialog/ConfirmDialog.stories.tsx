import type { Meta, StoryObj } from "@storybook/react";
import { ConfirmDialogProvider, useConfirm } from "./ConfirmDialog";
import { Button } from "../Button/Button";

const meta: Meta = {
  title: "UI/ConfirmDialog",
  parameters: {
    layout: "centered",
  },

  decorators: [
    (Story) => (
      <ConfirmDialogProvider>
        <Story />
      </ConfirmDialogProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

function ConfirmDialogDemo() {
  const confirm = useConfirm();

  const handleDelete = async () => {
    const confirmed = await confirm({
      title: "Delete Item",
      message: "Are you sure you want to delete this item? This action cannot be undone.",
      confirmLabel: "Delete",
      cancelLabel: "Cancel",
      variant: "destructive",
    });
    if (confirmed) {
      console.log("Item deleted");
    }
  };

  return (
    <Button variant="danger" onClick={handleDelete}>
      Delete Item
    </Button>
  );
}

export const Default: Story = {
  render: () => <ConfirmDialogDemo />,
};

function SimpleConfirm() {
  const confirm = useConfirm();

  const handleConfirm = async () => {
    const confirmed = await confirm({
      message: "Do you want to continue?",
    });
    if (confirmed) {
      console.log("Confirmed");
    }
  };

  return <Button onClick={handleConfirm}>Confirm Action</Button>;
}

export const Simple: Story = {
  render: () => <SimpleConfirm />,
};

function DestructiveConfirm() {
  const confirm = useConfirm();

  const handleConfirm = async () => {
    const confirmed = await confirm({
      title: "Remove API Key",
      message: "This will permanently revoke access for all applications using this key.",
      confirmLabel: "Revoke Key",
      variant: "destructive",
    });
    if (confirmed) {
      console.log("Key revoked");
    }
  };

  return (
    <Button variant="danger" onClick={handleConfirm}>
      Revoke API Key
    </Button>
  );
}

export const Destructive: Story = {
  render: () => <DestructiveConfirm />,
};
