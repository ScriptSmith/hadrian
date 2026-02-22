import type { Meta, StoryObj } from "@storybook/react";
import { ToastProvider, useToast } from "./Toast";
import { Button } from "../Button/Button";

const meta: Meta = {
  title: "UI/Toast",
  parameters: {
    layout: "centered",
  },

  decorators: [
    (Story) => (
      <ToastProvider>
        <Story />
      </ToastProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

function ToastDemo() {
  const { success, error, warning, info } = useToast();
  return (
    <div className="flex gap-2 flex-wrap">
      <Button onClick={() => success("Success", "Operation completed successfully.")}>
        Success Toast
      </Button>
      <Button onClick={() => error("Error", "Something went wrong.")}>Error Toast</Button>
      <Button onClick={() => warning("Warning", "Please review your changes.")}>
        Warning Toast
      </Button>
      <Button onClick={() => info("Info", "Here's some useful information.")}>Info Toast</Button>
    </div>
  );
}

export const Default: Story = {
  render: () => <ToastDemo />,
};

function ToastWithLongMessage() {
  const { success } = useToast();
  return (
    <Button
      onClick={() =>
        success(
          "Item Saved",
          "Your changes have been saved successfully. The item has been updated in the database and all related records have been synchronized."
        )
      }
    >
      Show Toast with Long Message
    </Button>
  );
}

export const LongMessage: Story = {
  render: () => <ToastWithLongMessage />,
};
