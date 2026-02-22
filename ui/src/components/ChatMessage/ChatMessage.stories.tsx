import type { Meta, StoryObj } from "@storybook/react";
import { expect, within } from "storybook/test";
import { ChatMessage } from "./ChatMessage";
import type { ChatMessage as ChatMessageType } from "../chat-types";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";

const meta: Meta<typeof ChatMessage> = {
  title: "Chat/ChatMessage",
  component: ChatMessage,
  parameters: {
    layout: "padded",
  },

  decorators: [
    (Story) => (
      <PreferencesProvider>
        <div style={{ maxWidth: 800 }}>
          <Story />
        </div>
      </PreferencesProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

const userMessage: ChatMessageType = {
  id: "1",
  role: "user",
  content: "Hello! Can you help me with a coding question?",
  timestamp: new Date(),
};

const assistantMessage: ChatMessageType = {
  id: "2",
  role: "assistant",
  content:
    "Of course! I'd be happy to help with your coding question. What would you like to know?",
  model: "gpt-4",
  timestamp: new Date(),
};

const markdownMessage: ChatMessageType = {
  id: "3",
  role: "assistant",
  content: `Here's an example of a simple React component:

\`\`\`typescript
function Greeting({ name }: { name: string }) {
  return <h1>Hello, {name}!</h1>;
}
\`\`\`

This component:
- Takes a \`name\` prop as a string
- Returns an \`<h1>\` element with a greeting

You can use it like this:

\`\`\`tsx
<Greeting name="World" />
\`\`\``,
  model: "claude-3",
  timestamp: new Date(),
};

const messageWithUsage: ChatMessageType = {
  id: "4",
  role: "assistant",
  content: "Here is my response with token usage tracking.",
  model: "gpt-4",
  timestamp: new Date(),
  usage: {
    inputTokens: 50,
    outputTokens: 100,
    totalTokens: 150,
    cost: 0.0025,
    cachedTokens: 10,
    reasoningTokens: 20,
  },
};

/**
 * Test: User message renders correctly with proper aria-label and content
 */
export const UserMessage: Story = {
  args: {
    message: userMessage,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify user message renders with correct aria-label
    const article = canvas.getByRole("article", { name: /your message/i });
    await expect(article).toBeInTheDocument();

    // Verify content is displayed
    await expect(canvas.getByText(userMessage.content)).toBeInTheDocument();

    // Verify avatar is present (contains User icon)
    const avatar = canvasElement.querySelector("svg");
    await expect(avatar).toBeInTheDocument();
  },
};

/**
 * Test: Assistant message renders correctly with proper aria-label
 */
export const AssistantMessage: Story = {
  args: {
    message: assistantMessage,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify assistant message renders with correct aria-label
    const article = canvas.getByRole("article", { name: /assistant message/i });
    await expect(article).toBeInTheDocument();

    // Verify content is displayed
    await expect(canvas.getByText(assistantMessage.content)).toBeInTheDocument();
  },
};

/**
 * Test: Streaming state shows aria-live region and blinking cursor
 */
export const StreamingMessage: Story = {
  args: {
    message: {
      ...assistantMessage,
      content: "I'm thinking about your question...",
    },
    isStreaming: true,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify streaming indicator has aria-live for accessibility
    const streamingContent = canvasElement.querySelector('[aria-live="polite"]');
    await expect(streamingContent).toBeInTheDocument();

    // Verify aria-busy is set during streaming
    const busyElement = canvasElement.querySelector('[aria-busy="true"]');
    await expect(busyElement).toBeInTheDocument();

    // Verify blinking cursor is present (screen reader hidden)
    const cursor = canvasElement.querySelector('[aria-hidden="true"]');
    await expect(cursor).toBeInTheDocument();

    // Verify screen reader text for streaming state
    await expect(canvas.getByText("Generating response...")).toBeInTheDocument();
  },
};

/**
 * Test: Markdown content renders code blocks and lists
 */
export const WithMarkdown: Story = {
  args: {
    message: markdownMessage,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify markdown content is rendered
    const article = canvas.getByRole("article", { name: /assistant message/i });
    await expect(article).toBeInTheDocument();

    // Verify code blocks are rendered (look for pre or code elements)
    const codeBlocks = canvasElement.querySelectorAll("pre, code");
    await expect(codeBlocks.length).toBeGreaterThan(0);

    // Verify list items are rendered
    const listItems = canvasElement.querySelectorAll("li");
    await expect(listItems.length).toBeGreaterThan(0);
  },
};

/**
 * Test: File preview renders file name for non-image files
 */
export const WithFile: Story = {
  args: {
    message: {
      ...userMessage,
      content: "Here is the file you requested:",
      files: [
        {
          id: "file-1",
          name: "document.pdf",
          type: "application/pdf",
          size: 1024000,
          base64: "",
        },
      ],
    },
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify file preview section exists
    await expect(canvas.getByText("document.pdf")).toBeInTheDocument();

    // Verify message content is also displayed
    await expect(canvas.getByText("Here is the file you requested:")).toBeInTheDocument();
  },
};

/**
 * Test: Image file preview renders with img tag
 */
export const WithImageFile: Story = {
  args: {
    message: {
      ...userMessage,
      content: "Check out this image:",
      files: [
        {
          id: "img-1",
          name: "screenshot.png",
          type: "image/png",
          size: 512000,
          base64:
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
          preview:
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
        },
      ],
    },
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify image preview is rendered with descriptive alt text
    const image = canvas.getByRole("img", { name: "Preview of screenshot.png" });
    await expect(image).toBeInTheDocument();

    // Verify file name is displayed
    await expect(canvas.getByText("screenshot.png")).toBeInTheDocument();
  },
};

/**
 * Test: Copy button is present and accessible
 * Note: We don't test actual clipboard functionality as Playwright requires
 * special permissions. We just verify the button exists and is accessible.
 */
export const CopyButtonAccessible: Story = {
  args: {
    message: assistantMessage,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Find the copy button by aria-label
    const copyButton = canvas.getByRole("button", { name: /copy message/i });
    await expect(copyButton).toBeInTheDocument();

    // Verify button is interactive
    await expect(copyButton).toBeEnabled();
  },
};

/**
 * Test: Token usage and cost are displayed for assistant messages
 */
export const WithTokenUsage: Story = {
  args: {
    message: messageWithUsage,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify token count is displayed
    await expect(canvas.getByText(/150 tokens/i)).toBeInTheDocument();

    // Verify cost is displayed
    await expect(canvas.getByText(/\$0\.0025/i)).toBeInTheDocument();
  },
};

/**
 * Test: Multiple messages in conversation render correctly
 */
export const Conversation: Story = {
  render: () => (
    <div className="space-y-2">
      <ChatMessage message={userMessage} />
      <ChatMessage message={assistantMessage} />
      <ChatMessage
        message={{
          id: "3",
          role: "user",
          content: "Can you show me an example in TypeScript?",
          timestamp: new Date(),
        }}
      />
      <ChatMessage message={markdownMessage} />
    </div>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify all messages are rendered
    const articles = canvas.getAllByRole("article");
    await expect(articles).toHaveLength(4);

    // Verify user messages are on one side and assistant on other
    const userMessages = canvas.getAllByRole("article", { name: /your message/i });
    const assistantMessages = canvas.getAllByRole("article", { name: /assistant message/i });
    await expect(userMessages).toHaveLength(2);
    await expect(assistantMessages).toHaveLength(2);
  },
};

/**
 * Test: Non-streaming state does NOT have aria-live or aria-busy
 */
export const NotStreamingNoAriaLive: Story = {
  args: {
    message: assistantMessage,
    isStreaming: false,
  },
  play: async ({ canvasElement }) => {
    // When not streaming, aria-live should not be set
    const ariaLiveElement = canvasElement.querySelector('[aria-live="polite"]');
    await expect(ariaLiveElement).not.toBeInTheDocument();

    // aria-busy should not be set
    const busyElement = canvasElement.querySelector('[aria-busy="true"]');
    await expect(busyElement).not.toBeInTheDocument();
  },
};

/**
 * Test: Fork button is present when onFork callback is provided
 */
export const WithForkButton: Story = {
  args: {
    message: userMessage,
    onFork: (messageId: string) => console.log("Fork from message:", messageId),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Find the fork button by aria-label
    const forkButton = canvas.getByRole("button", { name: /fork conversation from here/i });
    await expect(forkButton).toBeInTheDocument();

    // Verify button is interactive
    await expect(forkButton).toBeEnabled();
  },
};

/**
 * Test: Edit button is present when onSaveEdit callback is provided (user messages)
 */
export const WithEditButton: Story = {
  args: {
    message: userMessage,
    onSaveEdit: (messageId: string, newContent: string) =>
      console.log("Edit message:", messageId, newContent),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Find the edit button by aria-label
    const editButton = canvas.getByRole("button", { name: /edit message/i });
    await expect(editButton).toBeInTheDocument();

    // Verify button is interactive
    await expect(editButton).toBeEnabled();
  },
};

/**
 * Test: User message with both edit and fork buttons
 */
export const WithEditAndForkButtons: Story = {
  args: {
    message: userMessage,
    onSaveEdit: (messageId: string, newContent: string) =>
      console.log("Edit message:", messageId, newContent),
    onFork: (messageId: string) => console.log("Fork from message:", messageId),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify both buttons are present
    const editButton = canvas.getByRole("button", { name: /edit message/i });
    const forkButton = canvas.getByRole("button", { name: /fork conversation from here/i });

    await expect(editButton).toBeInTheDocument();
    await expect(forkButton).toBeInTheDocument();
  },
};

/**
 * Test: Regenerate button is present when onRegenerate callback is provided (user messages)
 */
export const WithRegenerateButton: Story = {
  args: {
    message: userMessage,
    onRegenerate: (messageId: string) => console.log("Regenerate from message:", messageId),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Find the regenerate button by aria-label
    const regenerateButton = canvas.getByRole("button", { name: /regenerate responses/i });
    await expect(regenerateButton).toBeInTheDocument();

    // Verify button is interactive
    await expect(regenerateButton).toBeEnabled();
  },
};

/**
 * Test: User message with all action buttons (regenerate, edit, fork)
 */
export const WithAllActionButtons: Story = {
  args: {
    message: userMessage,
    onRegenerate: (messageId: string) => console.log("Regenerate from message:", messageId),
    onSaveEdit: (messageId: string, newContent: string) =>
      console.log("Edit message:", messageId, newContent),
    onFork: (messageId: string) => console.log("Fork from message:", messageId),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify all buttons are present
    const regenerateButton = canvas.getByRole("button", { name: /regenerate responses/i });
    const editButton = canvas.getByRole("button", { name: /edit message/i });
    const forkButton = canvas.getByRole("button", { name: /fork conversation from here/i });

    await expect(regenerateButton).toBeInTheDocument();
    await expect(editButton).toBeInTheDocument();
    await expect(forkButton).toBeInTheDocument();
  },
};

/**
 * Test: User message with markdown content renders correctly
 */
export const UserMessageWithMarkdown: Story = {
  args: {
    message: {
      ...userMessage,
      content: `Can you help me with this **code** problem?

\`\`\`javascript
const x = 1;
console.log(x);
\`\`\`

I'm getting an \`error\` when running it.`,
    },
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify user message renders
    const article = canvas.getByRole("article", { name: /your message/i });
    await expect(article).toBeInTheDocument();

    // Verify code blocks are rendered (markdown is now applied to user messages)
    const codeBlocks = canvasElement.querySelectorAll("pre, code");
    await expect(codeBlocks.length).toBeGreaterThan(0);
  },
};
