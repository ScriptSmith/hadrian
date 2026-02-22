import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";
import { QuoteSelectionPopover } from "./QuoteSelectionPopover";

const meta: Meta<typeof QuoteSelectionPopover> = {
  title: "Chat/QuoteSelectionPopover",
  component: QuoteSelectionPopover,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

function DefaultStory() {
  const [quotedText, setQuotedText] = useState<string | null>(null);

  return (
    <div className="relative h-64 w-96">
      <div className="rounded border bg-card p-4">
        <p className="text-sm text-muted-foreground">
          The popover appears below. In practice, it shows when you select text in a chat message.
        </p>
        {quotedText && (
          <div className="mt-4 rounded border-l-2 border-primary bg-muted p-2">
            <p className="text-xs text-muted-foreground">Quoted:</p>
            <p className="text-sm">{quotedText}</p>
          </div>
        )}
      </div>

      <QuoteSelectionPopover
        isOpen={true}
        position={{ x: 192, y: 120 }}
        selectedText="This is the selected text"
        onQuote={(text) => setQuotedText(text)}
        onClose={() => {}}
      />
    </div>
  );
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function InteractiveStory() {
  const [isOpen, setIsOpen] = useState(false);
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const [selectedText, setSelectedText] = useState("");
  const [quotedText, setQuotedText] = useState<string | null>(null);

  const handleMouseUp = (e: React.MouseEvent) => {
    const selection = window.getSelection();
    const text = selection?.toString().trim() || "";

    if (text.length > 0) {
      setSelectedText(text);
      setPosition({ x: e.clientX, y: e.clientY });
      setIsOpen(true);
    }
  };

  return (
    <div className="w-96 space-y-4">
      {/* eslint-disable-next-line jsx-a11y/no-static-element-interactions -- story demo for text selection */}
      <div className="rounded border bg-card p-4" onMouseUp={handleMouseUp}>
        <p className="select-text text-sm">
          Try selecting some of this text to see the quote popover appear. You can select any
          portion of this paragraph and click the Quote button to add it to the quoted text section
          below.
        </p>
      </div>

      {quotedText && (
        <div className="rounded border-l-2 border-primary bg-muted p-3">
          <p className="text-xs font-medium text-muted-foreground">Quoted text:</p>
          <p className="mt-1 text-sm italic">&ldquo;{quotedText}&rdquo;</p>
        </div>
      )}

      <QuoteSelectionPopover
        isOpen={isOpen}
        position={position}
        selectedText={selectedText}
        onQuote={(text) => {
          setQuotedText(text);
          setIsOpen(false);
        }}
        onClose={() => setIsOpen(false)}
      />
    </div>
  );
}

export const Interactive: Story = {
  render: () => <InteractiveStory />,
};

export const NearEdge: Story = {
  render: () => (
    <div className="relative h-64 w-96">
      <p className="text-sm text-muted-foreground">
        The popover automatically adjusts position to stay within viewport bounds.
      </p>

      <QuoteSelectionPopover
        isOpen={true}
        position={{ x: 20, y: 30 }}
        selectedText="Text near corner"
        onQuote={() => {}}
        onClose={() => {}}
      />
    </div>
  ),
};
