import type { Meta, StoryObj } from "@storybook/react";
import { useState, useEffect } from "react";
import { StreamingMarkdown } from "./StreamingMarkdown";
import { PreferencesProvider } from "../../preferences/PreferencesProvider";

const meta: Meta<typeof StreamingMarkdown> = {
  title: "UI/StreamingMarkdown",
  component: StreamingMarkdown,
  parameters: {
    layout: "padded",
  },
  decorators: [
    (Story) => (
      <PreferencesProvider>
        <Story />
      </PreferencesProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Static: Story = {
  args: {
    content: `# Hello World

This is a **bold** statement and this is *italic*.

Here's a [link](https://example.com) to somewhere.`,
    isStreaming: false,
  },
};

export const Streaming: Story = {
  args: {
    content: `# Streaming Content

This content is being streamed. The cursor should be visible and copy buttons disabled.

\`\`\`javascript
function hello() {
  console.log("world");
}
\`\`\``,
    isStreaming: true,
  },
};

export const StreamingIncomplete: Story = {
  args: {
    content: `# Incomplete Markdown

This demonstrates streaming with unterminated blocks:

\`\`\`typescript
function incomplete() {
  // This code block is not closed yet...
  const x = 1;`,
    isStreaming: true,
  },
};

// Content for simulated streaming
const SIMULATED_CONTENT = `# API Response

Let me explain the concept of **async/await** in JavaScript.

## What is Async/Await?

Async/await is syntactic sugar built on top of Promises. It allows you to write asynchronous code that looks synchronous.

### Example

\`\`\`typescript
async function fetchData(url: string): Promise<Data> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error('Failed to fetch');
  }
  return response.json();
}
\`\`\`

> **Note:** Always handle errors in async functions!

### Benefits

1. Cleaner syntax
2. Easier debugging
3. Better error handling with try/catch

That's the basics of async/await!`;

// Simulates actual streaming behavior
function SimulatedStream() {
  const [content, setContent] = useState("");
  const [isStreaming, setIsStreaming] = useState(true);

  useEffect(() => {
    let index = 0;
    const interval = setInterval(() => {
      if (index < SIMULATED_CONTENT.length) {
        // Add 1-5 characters at a time to simulate token streaming
        const chunkSize = Math.floor(Math.random() * 5) + 1;
        const nextIndex = Math.min(index + chunkSize, SIMULATED_CONTENT.length);
        setContent(SIMULATED_CONTENT.slice(0, nextIndex));
        index = nextIndex;
      } else {
        setIsStreaming(false);
        clearInterval(interval);
      }
    }, 20);

    return () => clearInterval(interval);
  }, []);

  return <StreamingMarkdown content={content} isStreaming={isStreaming} />;
}

export const SimulatedStreaming: Story = {
  render: () => <SimulatedStream />,
};
