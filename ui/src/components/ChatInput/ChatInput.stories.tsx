import type { Meta, StoryObj } from "@storybook/react";
import { expect, userEvent, within, fn } from "storybook/test";
import { ChatInput } from "./ChatInput";
import { TooltipProvider } from "../Tooltip/Tooltip";
import { ConfigProvider } from "@/config/ConfigProvider";
import { useChatUIStore } from "@/stores/chatUIStore";

const meta: Meta<typeof ChatInput> = {
  title: "Chat/ChatInput",
  component: ChatInput,
  parameters: {
    layout: "centered",
  },

  decorators: [
    (Story) => (
      <ConfigProvider>
        <TooltipProvider>
          <div style={{ width: 600 }}>
            <Story />
          </div>
        </TooltipProvider>
      </ConfigProvider>
    ),
  ],
  args: {
    onSend: fn(),
    onStop: fn(),
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * Test: Default input renders with placeholder and send button
 */
export const Default: Story = {
  args: {
    placeholder: "Type a message...",
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify textarea exists with placeholder
    const textarea = canvas.getByPlaceholderText("Type a message...");
    await expect(textarea).toBeInTheDocument();

    // Verify send button exists and is disabled (no content)
    const sendButton = canvas.getByRole("button", { name: /send/i });
    await expect(sendButton).toBeInTheDocument();
    await expect(sendButton).toBeDisabled();
  },
};

/**
 * Test: Settings button appears when onSettingsClick is provided
 */
export const WithSettings: Story = {
  args: {
    placeholder: "Type a message...",
    onSettingsClick: fn(),
  },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);

    // Find all buttons - settings button is the first icon button (before Send)
    const buttons = canvas.getAllByRole("button");

    // Filter to icon buttons (not the Send button)
    const settingsButton = buttons.find((btn) => {
      const svg = btn.querySelector("svg");
      return svg && !btn.textContent?.includes("Send");
    });

    await expect(settingsButton).toBeInTheDocument();

    // Click settings button
    await userEvent.click(settingsButton!);

    // Verify callback was called
    await expect(args.onSettingsClick).toHaveBeenCalled();
  },
};

/**
 * Test: Settings button shows active state when hasSystemPrompt is true
 */
export const WithActiveSystemPrompt: Story = {
  args: {
    placeholder: "Type a message...",
    onSettingsClick: fn(),
    hasSystemPrompt: true,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Find icon buttons
    const buttons = canvas.getAllByRole("button");
    const settingsButton = buttons.find((btn) => {
      const svg = btn.querySelector("svg");
      return svg && !btn.textContent?.includes("Send");
    });

    await expect(settingsButton).toBeInTheDocument();

    // The settings button should have text-primary class when system prompt is active
    await expect(settingsButton?.className).toContain("text-primary");
  },
};

/**
 * Test: Custom placeholder text is displayed
 */
export const CustomPlaceholder: Story = {
  args: {
    placeholder: "Ask me anything...",
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify custom placeholder
    const textarea = canvas.getByPlaceholderText("Ask me anything...");
    await expect(textarea).toBeInTheDocument();
  },
};

/**
 * Test: Input is disabled and send button is disabled when disabled prop is true
 */
export const Disabled: Story = {
  args: {
    disabled: true,
    placeholder: "Select a model to start chatting...",
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify textarea is disabled
    const textarea = canvas.getByPlaceholderText("Select a model to start chatting...");
    await expect(textarea).toBeDisabled();

    // Verify send button is disabled
    const sendButton = canvas.getByRole("button", { name: /send/i });
    await expect(sendButton).toBeDisabled();
  },
};

/**
 * Test: No models selected shows prominent overlay hint
 */
export const NoModelsSelected: Story = {
  args: {
    disabled: true,
    noModelsSelected: true,
    placeholder: "Select a model to start chatting...",
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify the overlay hint is visible
    await expect(canvas.getByText("Select a model above to start chatting")).toBeInTheDocument();

    // Verify textarea is still disabled underneath
    const textarea = canvas.getByPlaceholderText("Select a model to start chatting...");
    await expect(textarea).toBeDisabled();
  },
};

/**
 * Test: During streaming, send button shows "Stop" and is enabled
 */
export const Streaming: Story = {
  args: {
    isStreaming: true,
  },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);

    // When streaming, button should show "Stop" and be enabled
    const stopButton = canvas.getByRole("button", { name: /stop/i });
    await expect(stopButton).toBeInTheDocument();
    await expect(stopButton).toBeEnabled();

    // Click stop button
    await userEvent.click(stopButton);

    // Verify onStop was called
    await expect(args.onStop).toHaveBeenCalled();
  },
};

/**
 * Test: Typing enables the send button
 */
export const TypingEnablesSend: Story = {
  args: {
    placeholder: "Type a message...",
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Initially, send button should be disabled
    const sendButton = canvas.getByRole("button", { name: /send/i });
    await expect(sendButton).toBeDisabled();

    // Type something
    const textarea = canvas.getByPlaceholderText("Type a message...");
    await userEvent.type(textarea, "Hello, world!");

    // Now send button should be enabled
    await expect(sendButton).toBeEnabled();
  },
};

/**
 * Test: Pressing Enter (without Shift) submits the message
 */
export const EnterSubmits: Story = {
  args: {
    placeholder: "Type a message...",
    onSend: fn(),
  },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);

    const textarea = canvas.getByPlaceholderText("Type a message...");

    // Type a message
    await userEvent.type(textarea, "Hello!");

    // Press Enter to submit
    await userEvent.keyboard("{Enter}");

    // Verify onSend was called with the message content
    await expect(args.onSend).toHaveBeenCalledWith("Hello!", []);

    // Verify input was cleared after send
    await expect(textarea).toHaveValue("");
  },
};

/**
 * Test: Shift+Enter adds a newline instead of submitting
 */
export const ShiftEnterNewline: Story = {
  args: {
    placeholder: "Type a message...",
    onSend: fn(),
  },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);

    const textarea = canvas.getByPlaceholderText("Type a message...");

    // Type a message
    await userEvent.type(textarea, "Line 1");

    // Press Shift+Enter
    await userEvent.keyboard("{Shift>}{Enter}{/Shift}");

    // Type more
    await userEvent.type(textarea, "Line 2");

    // onSend should NOT have been called
    await expect(args.onSend).not.toHaveBeenCalled();

    // Textarea should contain both lines
    await expect(textarea).toHaveValue("Line 1\nLine 2");
  },
};

/**
 * Test: Empty message cannot be submitted
 */
export const EmptyMessageNotSubmitted: Story = {
  args: {
    placeholder: "Type a message...",
    onSend: fn(),
  },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);

    const textarea = canvas.getByPlaceholderText("Type a message...");

    // Click in textarea
    await userEvent.click(textarea);

    // Press Enter without typing anything
    await userEvent.keyboard("{Enter}");

    // onSend should NOT have been called
    await expect(args.onSend).not.toHaveBeenCalled();
  },
};

/**
 * Test: History mode toggle appears when hasMultipleModels is true
 */
export const WithHistoryModeToggle: Story = {
  args: {
    placeholder: "Type a message...",
    hasMultipleModels: true,
    historyMode: "all",
    onHistoryModeChange: fn(),
  },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);

    // Find all icon buttons (buttons with SVG but not Send text)
    const buttons = canvas.getAllByRole("button");
    const iconButtons = buttons.filter((btn) => {
      const svg = btn.querySelector("svg");
      return svg && !btn.textContent?.includes("Send") && !btn.textContent?.includes("Stop");
    });

    // History mode toggle should be present (it's the only icon button when no settings)
    await expect(iconButtons.length).toBeGreaterThan(0);
    const toggleButton = iconButtons[0];

    // Click to toggle
    await userEvent.click(toggleButton);

    // Verify callback was called with the opposite mode
    await expect(args.onHistoryModeChange).toHaveBeenCalledWith("same-model");
  },
};

/**
 * Test: History mode toggle shows Split icon when in "same-model" mode
 */
export const WithHistoryModeSameModel: Story = {
  args: {
    placeholder: "Type a message...",
    hasMultipleModels: true,
    historyMode: "same-model",
    onHistoryModeChange: fn(),
  },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);

    // Find the toggle button (should have Split icon in same-model mode)
    const buttons = canvas.getAllByRole("button");
    const iconButtons = buttons.filter((btn) => {
      const svg = btn.querySelector("svg");
      return svg && !btn.textContent?.includes("Send") && !btn.textContent?.includes("Stop");
    });

    await expect(iconButtons.length).toBeGreaterThan(0);
    const toggleButton = iconButtons[0];

    // Verify the button has primary color (indicating active state)
    await expect(toggleButton).toHaveClass("text-primary");

    // Click to toggle back to "all"
    await userEvent.click(toggleButton);

    // Verify callback was called with "all"
    await expect(args.onHistoryModeChange).toHaveBeenCalledWith("all");
  },
};

/**
 * Test: ToolsBar appears when onEnabledToolsChange is provided
 */
export const WithToolsBar: Story = {
  args: {
    placeholder: "Type a message...",
    enabledTools: [],
    onEnabledToolsChange: fn(),
    vectorStoreIds: [],
    onVectorStoreIdsChange: fn(),
    vectorStoreOwnerType: "user",
    vectorStoreOwnerId: "user-123",
  },
  play: async ({ canvasElement }) => {
    // ToolsBar should be visible (wrench icon)
    const wrenchIcon = canvasElement.querySelector("svg.lucide-wrench");
    await expect(wrenchIcon).toBeInTheDocument();
  },
};

/**
 * Test: ToolsBar shows enabled tools
 */
export const WithEnabledTools: Story = {
  args: {
    placeholder: "Type a message...",
    enabledTools: ["code_interpreter", "web_search"],
    onEnabledToolsChange: fn(),
    vectorStoreIds: [],
    onVectorStoreIdsChange: fn(),
    vectorStoreOwnerType: "user",
    vectorStoreOwnerId: "user-123",
  },
  play: async ({ canvasElement }) => {
    // Wrench icon should still be present
    const wrenchIcon = canvasElement.querySelector("svg.lucide-wrench");
    await expect(wrenchIcon).toBeInTheDocument();

    // With enabled tools, the bar should show tool icons
    // The bar uses primary color for enabled tools
    const toolsBar = canvasElement.querySelector('[class*="text-primary"]');
    await expect(toolsBar).toBeInTheDocument();
  },
};

/**
 * Test: Quoted text is inserted as markdown blockquote
 */
export const WithQuotedText: Story = {
  args: {
    placeholder: "Type a message...",
    onSend: fn(),
  },
  decorators: [
    (Story) => {
      // Set quoted text in the store before rendering
      useChatUIStore.getState().setQuotedText({
        messageId: "msg-1",
        instanceId: "inst-1",
        text: "This is the quoted text from a previous message",
      });
      return (
        <ConfigProvider>
          <TooltipProvider>
            <div style={{ width: 600 }}>
              <Story />
            </div>
          </TooltipProvider>
        </ConfigProvider>
      );
    },
  ],
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Wait for the effect to run and insert the blockquote
    await new Promise((r) => setTimeout(r, 100));

    // Verify textarea contains the blockquote
    const textarea = canvas.getByPlaceholderText("Type a message...");
    await expect(textarea).toHaveValue("> This is the quoted text from a previous message\n\n");
  },
};

/**
 * Test: Multi-line quoted text preserves formatting
 */
export const WithMultiLineQuote: Story = {
  args: {
    placeholder: "Type a message...",
    onSend: fn(),
  },
  decorators: [
    (Story) => {
      useChatUIStore.getState().setQuotedText({
        messageId: "msg-1",
        text: "Line one\nLine two\nLine three",
      });
      return (
        <ConfigProvider>
          <TooltipProvider>
            <div style={{ width: 600 }}>
              <Story />
            </div>
          </TooltipProvider>
        </ConfigProvider>
      );
    },
  ],
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    await new Promise((r) => setTimeout(r, 100));

    // Verify multi-line blockquote formatting
    const textarea = canvas.getByPlaceholderText("Type a message...");
    await expect(textarea).toHaveValue("> Line one\n> Line two\n> Line three\n\n");
  },
};
