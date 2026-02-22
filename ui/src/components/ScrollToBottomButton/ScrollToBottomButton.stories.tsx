import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react";
import { ScrollToBottomButton } from "./ScrollToBottomButton";

const meta: Meta<typeof ScrollToBottomButton> = {
  title: "Chat/ScrollToBottomButton",
  component: ScrollToBottomButton,
  parameters: {
    layout: "centered",
  },
  argTypes: {
    visible: {
      control: "boolean",
      description: "Whether the button is visible",
    },
    onClick: {
      action: "clicked",
      description: "Callback when button is clicked",
    },
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Visible: Story = {
  args: {
    visible: true,
  },
};

export const Hidden: Story = {
  args: {
    visible: false,
  },
};

export const Interactive: Story = {
  render: function InteractiveStory() {
    const [visible, setVisible] = useState(true);
    const [scrolled, setScrolled] = useState(false);

    const handleClick = () => {
      setScrolled(true);
      setVisible(false);
      setTimeout(() => setScrolled(false), 1000);
    };

    const handleScrollUp = () => {
      setVisible(true);
    };

    return (
      <div className="flex flex-col items-center gap-4">
        <p className="text-sm text-muted-foreground">
          {scrolled
            ? "Scrolled to bottom!"
            : visible
              ? "Button is visible - click to scroll"
              : "Button is hidden"}
        </p>
        <div className="relative w-64 h-32 border border-border rounded-lg bg-muted/20">
          <ScrollToBottomButton
            visible={visible}
            onClick={handleClick}
            className="absolute bottom-2 right-2"
          />
        </div>
        {!visible && (
          <button onClick={handleScrollUp} className="text-xs text-primary hover:underline">
            Simulate scroll up
          </button>
        )}
      </div>
    );
  },
};

export const InContainer: Story = {
  render: function InContainerStory() {
    return (
      <div className="relative w-80 h-48 border border-border rounded-lg bg-background overflow-hidden">
        <div className="p-4 space-y-2">
          <p className="text-sm">This simulates how the button appears in the chat view.</p>
          <p className="text-sm text-muted-foreground">
            The button floats at the bottom-right corner.
          </p>
        </div>
        <ScrollToBottomButton
          visible={true}
          onClick={() => {}}
          className="absolute bottom-4 right-4"
        />
      </div>
    );
  },
};
