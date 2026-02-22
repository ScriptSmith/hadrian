import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { http, HttpResponse, delay } from "msw";
import { useState } from "react";
import { ConversationSettingsModal } from "./ConversationSettingsModal";
import { Button } from "../Button/Button";
import type { VectorStore, Organization, Prompt } from "@/api/generated/types.gen";

// Mock organizations
const mockOrganizations: Organization[] = [
  {
    id: "org_001",
    name: "Acme Corp",
    slug: "acme-corp",
    owner_id: "user_001",
    settings: {},
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
];

// Mock prompts
const mockPrompts: Prompt[] = [
  {
    id: "prompt_001",
    name: "Code Review Assistant",
    description: "Reviews code for best practices and potential issues",
    content:
      "You are an expert code reviewer. Analyze the provided code for best practices, potential bugs, and performance issues.",
    owner_type: "organization",
    owner_id: "org_001",
    metadata: null,
    created_at: "2024-12-01T00:00:00Z",
    updated_at: "2024-12-14T00:00:00Z",
  },
  {
    id: "prompt_002",
    name: "Technical Writer",
    description: "Helps write clear technical documentation",
    content:
      "You are a technical writer who creates clear, concise documentation. Focus on accuracy and readability.",
    owner_type: "organization",
    owner_id: "org_001",
    metadata: null,
    created_at: "2024-12-05T00:00:00Z",
    updated_at: "2024-12-10T00:00:00Z",
  },
  {
    id: "prompt_003",
    name: "Creative Brainstormer",
    description: null,
    content:
      "You are a creative assistant helping generate ideas. Think outside the box and suggest innovative solutions.",
    owner_type: "organization",
    owner_id: "org_001",
    metadata: null,
    created_at: "2024-12-10T00:00:00Z",
    updated_at: "2024-12-10T00:00:00Z",
  },
];

// Mock vector store data
const mockVectorStores: VectorStore[] = [
  {
    id: "vs_001",
    name: "Product Documentation",
    description: "Technical documentation for all products",
    object: "vector_store",
    status: "completed",
    owner_type: "user",
    owner_id: "user_123",
    embedding_model: "text-embedding-3-small",
    embedding_dimensions: 1536,
    usage_bytes: 2500000,
    file_counts: {
      in_progress: 0,
      completed: 15,
      failed: 0,
      cancelled: 0,
      total: 15,
    },
    created_at: "2024-12-01T10:00:00Z",
    updated_at: "2024-12-14T15:30:00Z",
  },
  {
    id: "vs_002",
    name: "Customer Support KB",
    description: "Knowledge base for customer support team",
    object: "vector_store",
    status: "completed",
    owner_type: "user",
    owner_id: "user_123",
    embedding_model: "text-embedding-3-small",
    embedding_dimensions: 1536,
    usage_bytes: 5000000,
    file_counts: {
      in_progress: 0,
      completed: 42,
      failed: 2,
      cancelled: 0,
      total: 44,
    },
    created_at: "2024-11-15T08:00:00Z",
    updated_at: "2024-12-10T12:00:00Z",
  },
  {
    id: "vs_003",
    name: "Legal Documents",
    description: "Contracts and legal documentation",
    object: "vector_store",
    status: "in_progress",
    owner_type: "user",
    owner_id: "user_123",
    embedding_model: "text-embedding-ada-002",
    embedding_dimensions: 1536,
    usage_bytes: 1000000,
    file_counts: {
      in_progress: 5,
      completed: 8,
      failed: 0,
      cancelled: 0,
      total: 13,
    },
    created_at: "2024-12-14T09:00:00Z",
    updated_at: "2024-12-14T09:30:00Z",
  },
];

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      staleTime: Infinity,
    },
  },
});

const meta: Meta<typeof ConversationSettingsModal> = {
  title: "Chat/ConversationSettingsModal",
  component: ConversationSettingsModal,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <Story />
      </QueryClientProvider>
    ),
  ],
  parameters: {
    layout: "centered",
    msw: {
      handlers: [
        http.get("/v1/vector_stores", async () => {
          await delay(300);
          return HttpResponse.json({
            object: "list",
            data: mockVectorStores,
            pagination: {
              limit: 100,
              has_more: false,
            },
          });
        }),
        // Mock organizations list
        http.get("/api/admin/v1/organizations", async () => {
          await delay(200);
          return HttpResponse.json({
            data: mockOrganizations,
            pagination: { limit: 100, has_more: false },
          });
        }),
        // Mock prompts list
        http.get("/api/admin/v1/organizations/:org_slug/prompts", async () => {
          await delay(200);
          return HttpResponse.json({
            data: mockPrompts,
            pagination: { limit: 100, has_more: false },
          });
        }),
        // Mock create prompt
        http.post("/api/admin/v1/prompts", async ({ request }) => {
          await delay(500);
          const body = (await request.json()) as Record<string, unknown>;
          const newPrompt: Prompt = {
            id: "prompt_new",
            name: body.name as string,
            description: (body.description as string) || null,
            content: body.content as string,
            owner_type: "organization",
            owner_id: "org_001",
            metadata: null,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(newPrompt, { status: 201 });
        }),
      ],
    },
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

function DefaultStory() {
  const [open, setOpen] = useState(true);
  const [systemPrompt, setSystemPrompt] = useState("");

  return (
    <>
      <Button onClick={() => setOpen(true)}>Open Settings</Button>
      <ConversationSettingsModal
        open={open}
        onClose={() => setOpen(false)}
        systemPrompt={systemPrompt}
        onSystemPromptChange={setSystemPrompt}
      />
    </>
  );
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function WithSystemPromptStory() {
  const [open, setOpen] = useState(true);
  const [systemPrompt, setSystemPrompt] = useState(
    "You are a helpful coding assistant specializing in TypeScript and React. Always provide clear explanations and working examples."
  );

  return (
    <>
      <Button onClick={() => setOpen(true)}>Open Settings</Button>
      <div className="mt-4 text-sm text-muted-foreground max-w-md">
        <p>
          This is the global system prompt. Individual models can override this via the settings
          icon on each model chip in the chat header.
        </p>
      </div>
      <ConversationSettingsModal
        open={open}
        onClose={() => setOpen(false)}
        systemPrompt={systemPrompt}
        onSystemPromptChange={setSystemPrompt}
      />
    </>
  );
}

export const WithSystemPrompt: Story = {
  render: () => <WithSystemPromptStory />,
};

function WithKnowledgeBaseStory() {
  const [open, setOpen] = useState(true);
  const [systemPrompt, setSystemPrompt] = useState(
    "You are a helpful assistant with access to knowledge bases."
  );
  const [vectorStoreIds, setVectorStoreIds] = useState<string[]>(["vs_001"]);

  return (
    <>
      <Button onClick={() => setOpen(true)}>Open Settings</Button>
      <div className="mt-4 text-sm text-muted-foreground">
        Selected vector stores: {vectorStoreIds.length > 0 ? vectorStoreIds.join(", ") : "(none)"}
      </div>
      <ConversationSettingsModal
        open={open}
        onClose={() => setOpen(false)}
        systemPrompt={systemPrompt}
        onSystemPromptChange={setSystemPrompt}
        vectorStoreIds={vectorStoreIds}
        onVectorStoreIdsChange={setVectorStoreIds}
        vectorStoreOwnerType="user"
        vectorStoreOwnerId="user_123"
      />
    </>
  );
}

export const WithKnowledgeBase: Story = {
  render: () => <WithKnowledgeBaseStory />,
};

function WithKnowledgeBaseEmptyStory() {
  const [open, setOpen] = useState(true);
  const [systemPrompt, setSystemPrompt] = useState("");
  const [vectorStoreIds, setVectorStoreIds] = useState<string[]>([]);

  return (
    <>
      <Button onClick={() => setOpen(true)}>Open Settings</Button>
      <ConversationSettingsModal
        open={open}
        onClose={() => setOpen(false)}
        systemPrompt={systemPrompt}
        onSystemPromptChange={setSystemPrompt}
        vectorStoreIds={vectorStoreIds}
        onVectorStoreIdsChange={setVectorStoreIds}
        vectorStoreOwnerType="user"
        vectorStoreOwnerId="user_123"
      />
    </>
  );
}

export const WithKnowledgeBaseEmpty: Story = {
  render: () => <WithKnowledgeBaseEmptyStory />,
};

function WithClientSideRAGStory() {
  const [open, setOpen] = useState(true);
  const [systemPrompt, setSystemPrompt] = useState(
    "You are a helpful assistant with access to knowledge bases."
  );
  const [vectorStoreIds, setVectorStoreIds] = useState<string[]>(["vs_001", "vs_002"]);
  const [clientSideRAG, setClientSideRAG] = useState(true);

  return (
    <>
      <Button onClick={() => setOpen(true)}>Open Settings</Button>
      <div className="mt-4 text-sm text-muted-foreground">
        <div>Selected vector stores: {vectorStoreIds.join(", ")}</div>
        <div>Client-side RAG: {clientSideRAG ? "enabled" : "disabled"}</div>
      </div>
      <ConversationSettingsModal
        open={open}
        onClose={() => setOpen(false)}
        systemPrompt={systemPrompt}
        onSystemPromptChange={setSystemPrompt}
        vectorStoreIds={vectorStoreIds}
        onVectorStoreIdsChange={setVectorStoreIds}
        vectorStoreOwnerType="user"
        vectorStoreOwnerId="user_123"
        clientSideRAG={clientSideRAG}
        onClientSideRAGChange={setClientSideRAG}
      />
    </>
  );
}

export const WithClientSideRAG: Story = {
  render: () => <WithClientSideRAGStory />,
};

function WithPromptTemplatesStory() {
  const [open, setOpen] = useState(true);
  const [systemPrompt, setSystemPrompt] = useState(
    "You are an expert code reviewer. Analyze the provided code for best practices."
  );

  return (
    <>
      <Button onClick={() => setOpen(true)}>Open Settings</Button>
      <div className="mt-4 text-sm text-muted-foreground max-w-md">
        <p>This story demonstrates:</p>
        <ul className="list-disc list-inside mt-2 space-y-1">
          <li>Templates dropdown to load saved prompts</li>
          <li>Save button to create new templates</li>
          <li>Templates are loaded from the mock API</li>
        </ul>
      </div>
      <ConversationSettingsModal
        open={open}
        onClose={() => setOpen(false)}
        systemPrompt={systemPrompt}
        onSystemPromptChange={setSystemPrompt}
      />
    </>
  );
}

export const WithPromptTemplates: Story = {
  render: () => <WithPromptTemplatesStory />,
};

function WithDebugOptionsStory() {
  const [open, setOpen] = useState(true);
  const [systemPrompt, setSystemPrompt] = useState("");
  const [captureRawSSEEvents, setCaptureRawSSEEvents] = useState(false);

  return (
    <>
      <Button onClick={() => setOpen(true)}>Open Settings</Button>
      <div className="mt-4 text-sm text-muted-foreground">
        <div>Capture SSE events: {captureRawSSEEvents ? "enabled" : "disabled"}</div>
      </div>
      <ConversationSettingsModal
        open={open}
        onClose={() => setOpen(false)}
        systemPrompt={systemPrompt}
        onSystemPromptChange={setSystemPrompt}
        captureRawSSEEvents={captureRawSSEEvents}
        onCaptureRawSSEEventsChange={setCaptureRawSSEEvents}
      />
    </>
  );
}

export const WithDebugOptions: Story = {
  render: () => <WithDebugOptionsStory />,
};

function AllFeaturesStory() {
  const [open, setOpen] = useState(true);
  const [systemPrompt, setSystemPrompt] = useState(
    "You are a helpful assistant with access to documentation and support knowledge."
  );
  const [vectorStoreIds, setVectorStoreIds] = useState<string[]>(["vs_001", "vs_002"]);
  const [clientSideRAG, setClientSideRAG] = useState(false);
  const [captureRawSSEEvents, setCaptureRawSSEEvents] = useState(false);

  return (
    <>
      <Button onClick={() => setOpen(true)}>Open Settings</Button>
      <div className="mt-4 text-sm text-muted-foreground max-w-md">
        <p>This story shows all settings sections:</p>
        <ul className="list-disc list-inside mt-2 space-y-1">
          <li>System Prompt with templates</li>
          <li>Knowledge Base (vector stores)</li>
          <li>Response Actions</li>
          <li>Debug Options</li>
        </ul>
        <p className="mt-2 text-xs">
          Note: Per-model parameters are now configured via the settings icon on each model chip in
          the chat header.
        </p>
      </div>
      <ConversationSettingsModal
        open={open}
        onClose={() => setOpen(false)}
        systemPrompt={systemPrompt}
        onSystemPromptChange={setSystemPrompt}
        vectorStoreIds={vectorStoreIds}
        onVectorStoreIdsChange={setVectorStoreIds}
        vectorStoreOwnerType="user"
        vectorStoreOwnerId="user_123"
        clientSideRAG={clientSideRAG}
        onClientSideRAGChange={setClientSideRAG}
        captureRawSSEEvents={captureRawSSEEvents}
        onCaptureRawSSEEventsChange={setCaptureRawSSEEvents}
      />
    </>
  );
}

export const AllFeatures: Story = {
  render: () => <AllFeaturesStory />,
};
