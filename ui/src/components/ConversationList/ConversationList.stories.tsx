import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";
import { ConversationList } from "./ConversationList";
import { ConfirmDialogProvider } from "../ConfirmDialog/ConfirmDialog";
import type { Conversation } from "../chat-types";

const meta: Meta<typeof ConversationList> = {
  title: "Chat/ConversationList",
  component: ConversationList,
  parameters: {
    layout: "padded",
  },

  decorators: [
    (Story) => (
      <ConfirmDialogProvider>
        <div style={{ width: 280, height: 500, border: "1px solid var(--border)" }}>
          <Story />
        </div>
      </ConfirmDialogProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

const now = new Date();
const mockConversations: Conversation[] = [
  {
    id: "1",
    title: "React hooks discussion",
    messages: [],
    models: ["anthropic/claude-3-opus"],
    createdAt: now,
    updatedAt: now,
  },
  {
    id: "2",
    title: "TypeScript best practices",
    messages: [],
    models: ["openai/gpt-4"],
    createdAt: now,
    updatedAt: new Date(now.getTime() - 2 * 60 * 60 * 1000),
  },
  {
    id: "3",
    title: "API design patterns",
    messages: [],
    models: ["anthropic/claude-3-sonnet"],
    createdAt: now,
    updatedAt: new Date(now.getTime() - 24 * 60 * 60 * 1000),
  },
  {
    id: "4",
    title: "Database optimization",
    messages: [],
    models: ["openai/gpt-4-turbo"],
    createdAt: now,
    updatedAt: new Date(now.getTime() - 3 * 24 * 60 * 60 * 1000),
  },
  {
    id: "5",
    title: "Python machine learning",
    messages: [],
    models: ["google/gemini-pro"],
    createdAt: now,
    updatedAt: new Date(now.getTime() - 10 * 24 * 60 * 60 * 1000),
  },
];

function DefaultStory() {
  const [conversations, setConversations] = useState(mockConversations);
  const [currentId, setCurrentId] = useState<string | null>("1");

  return (
    <ConversationList
      conversations={conversations}
      currentConversationId={currentId}
      onSelect={setCurrentId}
      onNew={() => {
        const newConv: Conversation = {
          id: String(conversations.length + 1),
          title: "New conversation",
          messages: [],
          models: [],
          createdAt: new Date(),
          updatedAt: new Date(),
        };
        setConversations([newConv, ...conversations]);
        setCurrentId(newConv.id);
      }}
      onDelete={(id) => {
        setConversations(conversations.filter((c) => c.id !== id));
        if (currentId === id) setCurrentId(null);
      }}
      onRename={(id, title) => {
        setConversations(conversations.map((c) => (c.id === id ? { ...c, title } : c)));
      }}
      onRegenerateTitle={(id) => {
        // Simulate regenerating title with a random suffix
        setConversations(
          conversations.map((c) =>
            c.id === id ? { ...c, title: `Regenerated: ${c.title.slice(0, 20)}` } : c
          )
        );
      }}
      onFork={(id) => {
        const source = conversations.find((c) => c.id === id);
        if (!source) return;
        const forked: Conversation = {
          ...source,
          id: String(conversations.length + 1),
          title: `${source.title} (fork)`,
          createdAt: new Date(),
          updatedAt: new Date(),
        };
        setConversations([forked, ...conversations]);
        setCurrentId(forked.id);
      }}
    />
  );
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

export const Empty: Story = {
  render: () => (
    <ConversationList
      conversations={[]}
      currentConversationId={null}
      onSelect={() => {}}
      onNew={() => {}}
      onDelete={() => {}}
      onRename={() => {}}
      onRegenerateTitle={() => {}}
      onFork={() => {}}
    />
  ),
};

export const ManyConversations: Story = {
  render: () => {
    const manyConversations: Conversation[] = Array.from({ length: 20 }, (_, i) => ({
      id: String(i + 1),
      title: `Conversation ${i + 1}`,
      messages: [],
      models: ["anthropic/claude-3-opus"],
      createdAt: new Date(),
      updatedAt: new Date(now.getTime() - i * 24 * 60 * 60 * 1000),
    }));

    return (
      <ConversationList
        conversations={manyConversations}
        currentConversationId="1"
        onSelect={() => {}}
        onNew={() => {}}
        onDelete={() => {}}
        onRename={() => {}}
        onRegenerateTitle={() => {}}
        onFork={() => {}}
      />
    );
  },
};
