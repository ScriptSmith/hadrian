import type { Meta, StoryObj } from "@storybook/react";
import { MCPUIRenderer } from "./MCPUIRenderer";

const meta = {
  title: "Chat/MCPUIRenderer",
  component: MCPUIRenderer,
  parameters: {
    layout: "padded",
  },
} satisfies Meta<typeof MCPUIRenderer>;

export default meta;
type Story = StoryObj<typeof meta>;

export const HtmlResource: Story = {
  args: {
    resource: {
      uri: "mcp://server/html-resource",
      mimeType: "text/html",
      text: `
        <div style="padding: 16px; font-family: system-ui, sans-serif;">
          <h2 style="margin: 0 0 12px 0; color: #333;">Welcome</h2>
          <p style="color: #666; line-height: 1.5;">
            This is rendered HTML content from an MCP server resource.
          </p>
          <button style="margin-top: 12px; padding: 8px 16px; background: #3b82f6; color: white; border: none; border-radius: 4px; cursor: pointer;">
            Interactive Button
          </button>
        </div>
      `,
    },
  },
};

export const WithActionHandlers: Story = {
  args: {
    resource: {
      uri: "mcp://interactive/form",
      mimeType: "text/html",
      text: `
        <div style="padding: 16px; font-family: system-ui;">
          <h3>Interactive Form</h3>
          <p>Click the button to trigger an action.</p>
        </div>
      `,
    },
    actionHandlers: {
      onToolCall: async (toolName, params) => {
        console.log("Tool called:", toolName, params);
        return { result: "success" };
      },
      onPrompt: (prompt) => {
        console.log("Prompt requested:", prompt);
      },
    },
  },
};

export const ExternalUrl: Story = {
  args: {
    resource: {
      uri: "https://example.com",
      mimeType: "text/uri-list",
      text: "https://example.com",
    },
  },
};

export const CustomStyling: Story = {
  args: {
    resource: {
      uri: "mcp://styled/content",
      mimeType: "text/html",
      text: "<p style='color: green;'>Styled content</p>",
    },
    className: "rounded-lg border shadow-sm",
    style: { maxHeight: "200px" },
  },
};
