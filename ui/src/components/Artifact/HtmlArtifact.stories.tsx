import type { Meta, StoryObj } from "@storybook/react";
import { HtmlArtifact } from "./HtmlArtifact";
import type { Artifact } from "@/components/chat-types";

const meta = {
  title: "Chat/Artifacts/HtmlArtifact",
  component: HtmlArtifact,
  parameters: {
    layout: "padded",
  },
} satisfies Meta<typeof HtmlArtifact>;

export default meta;
type Story = StoryObj<typeof meta>;

const makeArtifact = (html: string): Artifact => ({
  id: "html-1",
  type: "html",
  title: "HTML Preview",
  data: { html },
});

export const SimpleHtml: Story = {
  args: {
    artifact: makeArtifact(`
      <h1>Hello World</h1>
      <p>This is a simple HTML preview.</p>
      <ul>
        <li>Item 1</li>
        <li>Item 2</li>
        <li>Item 3</li>
      </ul>
    `),
  },
};

export const StyledContent: Story = {
  args: {
    artifact: makeArtifact(`
      <div style="font-family: sans-serif; padding: 20px; background: #f0f0f0; border-radius: 8px;">
        <h2 style="color: #333;">Styled Card</h2>
        <p style="color: #666;">This content has inline styles applied.</p>
        <button style="background: #007bff; color: white; padding: 8px 16px; border: none; border-radius: 4px; cursor: pointer;">
          Click Me
        </button>
      </div>
    `),
  },
};

export const DataObject: Story = {
  args: {
    artifact: {
      id: "html-obj",
      type: "html",
      title: "HTML from Object",
      data: {
        content: "<p>HTML content from data.content property</p>",
      },
    },
  },
};
