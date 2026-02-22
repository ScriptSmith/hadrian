import type { Meta, StoryObj } from "@storybook/react";
import { MCPUIArtifact } from "./MCPUIArtifact";

const meta = {
  title: "Chat/Artifacts/MCPUIArtifact",
  component: MCPUIArtifact,
  parameters: {
    layout: "padded",
  },
} satisfies Meta<typeof MCPUIArtifact>;

export default meta;
type Story = StoryObj<typeof meta>;

export const HtmlContent: Story = {
  args: {
    data: {
      uri: "mcp://server/resource",
      mimeType: "text/html",
      text: `
        <div style="padding: 16px; font-family: sans-serif;">
          <h3 style="margin: 0 0 8px 0;">MCP UI Content</h3>
          <p style="color: #666; margin: 0;">This is rendered from an MCP server response.</p>
        </div>
      `,
      serverName: "weather-server",
      toolName: "get_forecast",
    },
  },
};

export const WithServerInfo: Story = {
  args: {
    data: {
      uri: "mcp://database/query-result",
      mimeType: "text/html",
      text: `
        <table style="width: 100%; border-collapse: collapse; font-family: sans-serif;">
          <thead>
            <tr style="background: #f5f5f5;">
              <th style="padding: 8px; border: 1px solid #ddd;">Name</th>
              <th style="padding: 8px; border: 1px solid #ddd;">Value</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td style="padding: 8px; border: 1px solid #ddd;">Item 1</td>
              <td style="padding: 8px; border: 1px solid #ddd;">100</td>
            </tr>
          </tbody>
        </table>
      `,
      serverName: "database-server",
      toolName: "run_query",
    },
  },
};

export const ExternalUrl: Story = {
  args: {
    data: {
      uri: "https://example.com",
      mimeType: "text/uri-list",
      text: "https://example.com",
      serverName: "web-server",
    },
  },
};
