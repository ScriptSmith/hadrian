import type { Meta, StoryObj } from "@storybook/react";
import { Markdown } from "./Markdown";
import { PreferencesProvider } from "../../preferences/PreferencesProvider";

const meta: Meta<typeof Markdown> = {
  title: "UI/Markdown",
  component: Markdown,
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

export const Basic: Story = {
  args: {
    content: `# Hello World

This is a **bold** statement and this is *italic*.

Here's a [link](https://example.com) to somewhere.`,
  },
};

export const CodeBlock: Story = {
  args: {
    content: `Here's some JavaScript code:

\`\`\`javascript
function greet(name) {
  console.log(\`Hello, \${name}!\`);
  return { greeting: "Hello", name };
}

greet("World");
\`\`\`

And some inline \`code\` as well.`,
  },
};

export const TypeScriptCode: Story = {
  args: {
    content: `## TypeScript Example

\`\`\`typescript
interface User {
  id: string;
  name: string;
  email: string;
  createdAt: Date;
}

async function fetchUser(id: string): Promise<User> {
  const response = await fetch(\`/api/users/\${id}\`);
  if (!response.ok) {
    throw new Error("Failed to fetch user");
  }
  return response.json();
}
\`\`\``,
  },
};

export const PythonCode: Story = {
  args: {
    content: `## Python Example

\`\`\`python
from dataclasses import dataclass
from typing import Optional

@dataclass
class Message:
    role: str
    content: str
    model: Optional[str] = None

def create_chat_completion(messages: list[Message]) -> str:
    """Generate a chat completion from messages."""
    return "Hello, World!"
\`\`\``,
  },
};

export const Lists: Story = {
  args: {
    content: `## Shopping List

- Apples
- Bananas
- Oranges

## Steps to Follow

1. First, gather all ingredients
2. Mix them together
3. Bake for 30 minutes
4. Enjoy!`,
  },
};

export const Blockquote: Story = {
  args: {
    content: `## Famous Quotes

> The only way to do great work is to love what you do.
>
> — Steve Jobs

And another one:

> In the middle of difficulty lies opportunity.
>
> — Albert Einstein`,
  },
};

export const Table: Story = {
  args: {
    content: `## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | /api/users | List all users |
| POST | /api/users | Create a user |
| GET | /api/users/:id | Get a user |
| DELETE | /api/users/:id | Delete a user |`,
  },
};

export const MermaidDiagram: Story = {
  args: {
    content: `## System Architecture

Here's a diagram showing the request flow through the gateway:

\`\`\`mermaid
flowchart LR
    Client --> Gateway
    Gateway --> Auth[Authentication]
    Auth --> Budget[Budget Check]
    Budget --> Router[Model Router]
    Router --> OpenAI
    Router --> Anthropic
    Router --> Google
\`\`\`

### Sequence Diagram

\`\`\`mermaid
sequenceDiagram
    participant C as Client
    participant G as Gateway
    participant P as Provider

    C->>G: POST /v1/chat/completions
    G->>G: Authenticate request
    G->>G: Check budget
    G->>P: Forward request
    P-->>G: Stream response
    G-->>C: Stream response
    G->>G: Track usage (async)
\`\`\``,
  },
};

export const ComplexDocument: Story = {
  args: {
    content: `# API Documentation

## Overview

This API provides access to the **Hadrian Gateway** functionality.

### Authentication

All requests require an API key in the header:

\`\`\`bash
curl -H "Authorization: Bearer gw_live_xxx" https://api.example.com/v1/chat
\`\`\`

### Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| /v1/chat/completions | POST | Create chat completion |
| /v1/models | GET | List available models |

### Example Request

\`\`\`typescript
const response = await fetch("https://api.example.com/v1/chat/completions", {
  method: "POST",
  headers: {
    "Content-Type": "application/json",
    "Authorization": "Bearer gw_live_xxx",
  },
  body: JSON.stringify({
    model: "openai/gpt-4",
    messages: [
      { role: "user", content: "Hello!" }
    ],
  }),
});
\`\`\`

> **Note:** Rate limits apply to all API requests.

### Error Handling

Errors are returned with the following structure:

\`\`\`json
{
  "error": {
    "message": "Invalid API key",
    "type": "authentication_error",
    "code": 401
  }
}
\`\`\``,
  },
};
