import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { http, HttpResponse } from "msw";

import type { Conversation } from "@/components/chat-types";

import { ForkConversationModal } from "./ForkConversationModal";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      staleTime: Infinity,
    },
  },
});

const mockOrganizations = [
  {
    id: "org-1",
    name: "Acme Corp",
    slug: "acme-corp",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "org-2",
    name: "Personal Org",
    slug: "personal",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
];

const mockProjects = [
  {
    id: "proj-1",
    name: "Frontend App",
    slug: "frontend-app",
    org_id: "org-1",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "proj-2",
    name: "Backend API",
    slug: "backend-api",
    org_id: "org-1",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "proj-3",
    name: "Side Project",
    slug: "side-project",
    org_id: "org-2",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
];

const handlers = [
  http.get("/api/admin/v1/organizations", () => {
    return HttpResponse.json({
      data: mockOrganizations,
      pagination: { limit: 100, has_more: false },
    });
  }),
  http.get("/api/admin/v1/organizations/:orgSlug/projects", ({ params }) => {
    const projects = mockProjects.filter((p) => {
      const org = mockOrganizations.find((o) => o.slug === params.orgSlug);
      return org && p.org_id === org.id;
    });
    return HttpResponse.json({
      data: projects,
      pagination: { limit: 100, has_more: false },
    });
  }),
];

// Helper to create mock conversations
function createMockConversation(overrides: Partial<Conversation> = {}): Conversation {
  return {
    id: "conv-1",
    title: "Discussion about API design",
    messages: [
      {
        id: "msg-1",
        role: "user",
        content: "Let's discuss API design patterns",
        timestamp: new Date("2024-01-15T10:00:00Z"),
      },
      {
        id: "msg-2",
        role: "assistant",
        model: "openai/gpt-4",
        content: "Sure! There are several patterns to consider...",
        timestamp: new Date("2024-01-15T10:00:05Z"),
      },
      {
        id: "msg-3",
        role: "assistant",
        model: "anthropic/claude-3-opus",
        content: "I'd recommend starting with REST principles...",
        timestamp: new Date("2024-01-15T10:00:06Z"),
      },
    ],
    models: ["openai/gpt-4", "anthropic/claude-3-opus"],
    createdAt: new Date("2024-01-15T10:00:00Z"),
    updatedAt: new Date("2024-01-15T10:05:00Z"),
    ...overrides,
  };
}

const meta = {
  title: "Components/ForkConversationModal",
  component: ForkConversationModal,
  parameters: {
    msw: { handlers },
  },
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <Story />
      </QueryClientProvider>
    ),
  ],
  argTypes: {
    onClose: { action: "onClose" },
    onFork: { action: "onFork" },
  },
  args: {
    open: true,
    onClose: () => {},
    onFork: () => {},
  },
} satisfies Meta<typeof ForkConversationModal>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * Default fork modal with multiple models
 */
export const Default: Story = {
  args: {
    conversation: createMockConversation(),
  },
};

/**
 * Fork from a specific message (partial fork)
 */
export const PartialFork: Story = {
  args: {
    conversation: createMockConversation({
      messages: [
        {
          id: "msg-1",
          role: "user",
          content: "First message",
          timestamp: new Date("2024-01-15T10:00:00Z"),
        },
        {
          id: "msg-2",
          role: "assistant",
          model: "openai/gpt-4",
          content: "First response",
          timestamp: new Date("2024-01-15T10:00:05Z"),
        },
        {
          id: "msg-3",
          role: "user",
          content: "Second message",
          timestamp: new Date("2024-01-15T10:01:00Z"),
        },
        {
          id: "msg-4",
          role: "assistant",
          model: "openai/gpt-4",
          content: "Second response",
          timestamp: new Date("2024-01-15T10:01:05Z"),
        },
        {
          id: "msg-5",
          role: "user",
          content: "Third message",
          timestamp: new Date("2024-01-15T10:02:00Z"),
        },
        {
          id: "msg-6",
          role: "assistant",
          model: "openai/gpt-4",
          content: "Third response",
          timestamp: new Date("2024-01-15T10:02:05Z"),
        },
      ],
    }),
    upToMessageId: "msg-4",
  },
};

/**
 * Single model conversation - model selection hidden
 */
export const SingleModel: Story = {
  args: {
    conversation: createMockConversation({
      models: ["openai/gpt-4"],
      messages: [
        {
          id: "msg-1",
          role: "user",
          content: "Hello",
          timestamp: new Date("2024-01-15T10:00:00Z"),
        },
        {
          id: "msg-2",
          role: "assistant",
          model: "openai/gpt-4",
          content: "Hi there!",
          timestamp: new Date("2024-01-15T10:00:05Z"),
        },
      ],
    }),
  },
};

/**
 * Many models - scrollable model list
 */
export const ManyModels: Story = {
  args: {
    conversation: createMockConversation({
      models: [
        "openai/gpt-4",
        "openai/gpt-4-turbo",
        "anthropic/claude-3-opus",
        "anthropic/claude-3-sonnet",
        "google/gemini-1.5-pro",
        "mistral/mistral-large",
      ],
    }),
  },
};

/**
 * Fork from project conversation - can move to personal
 */
export const FromProject: Story = {
  args: {
    conversation: createMockConversation({
      projectId: "proj-1",
      projectName: "Frontend App",
    }),
  },
};

/**
 * Long title that needs truncation
 */
export const LongTitle: Story = {
  args: {
    conversation: createMockConversation({
      title:
        "This is a very long conversation title that discusses multiple topics including API design, database architecture, and frontend performance optimization strategies",
    }),
  },
};

/**
 * Already has "(fork)" suffix - should not double it
 */
export const AlreadyForked: Story = {
  args: {
    conversation: createMockConversation({
      title: "Discussion about API design (fork)",
    }),
  },
};

/**
 * Empty conversation (edge case)
 */
export const EmptyConversation: Story = {
  args: {
    conversation: createMockConversation({
      messages: [],
    }),
  },
};
