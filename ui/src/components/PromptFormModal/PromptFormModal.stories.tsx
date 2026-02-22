import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { http, HttpResponse, delay } from "msw";
import { useState } from "react";
import { PromptFormModal } from "./PromptFormModal";
import { Button } from "../Button/Button";
import type { Organization, Prompt } from "@/api/generated/types.gen";

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
  {
    id: "org_002",
    name: "Tech Startup",
    slug: "tech-startup",
    owner_id: "user_001",
    settings: {},
    created_at: "2024-02-01T00:00:00Z",
    updated_at: "2024-02-01T00:00:00Z",
  },
];

// Mock existing prompt for editing
const mockPrompt: Prompt = {
  id: "prompt_001",
  name: "Code Review Assistant",
  description: "Reviews code for best practices and potential issues",
  content:
    "You are an expert code reviewer. Analyze the provided code for:\n1. Best practices\n2. Potential bugs\n3. Performance issues\n4. Security vulnerabilities\n\nProvide constructive feedback with specific suggestions for improvement.",
  owner_type: "organization",
  owner_id: "org_001",
  metadata: null,
  created_at: "2024-12-01T00:00:00Z",
  updated_at: "2024-12-14T00:00:00Z",
};

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      staleTime: Infinity,
    },
  },
});

const meta: Meta<typeof PromptFormModal> = {
  title: "Chat/PromptFormModal",
  component: PromptFormModal,
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
        // Mock organizations list
        http.get("/api/admin/v1/organizations", async () => {
          await delay(200);
          return HttpResponse.json({
            data: mockOrganizations,
            pagination: { limit: 100, has_more: false },
          });
        }),
        // Mock prompts list (empty initially)
        http.get("/api/admin/v1/organizations/:org_slug/prompts", async () => {
          await delay(200);
          return HttpResponse.json({
            data: [],
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
        // Mock update prompt
        http.patch("/api/admin/v1/prompts/:id", async ({ request }) => {
          await delay(500);
          const body = (await request.json()) as Record<string, unknown>;
          const updatedPrompt: Prompt = {
            ...mockPrompt,
            name: (body.name as string) || mockPrompt.name,
            description: (body.description as string) || mockPrompt.description,
            content: (body.content as string) || mockPrompt.content,
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(updatedPrompt);
        }),
      ],
    },
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

function CreateNewStory() {
  const [open, setOpen] = useState(true);

  return (
    <>
      <Button onClick={() => setOpen(true)}>Save as Template</Button>
      <PromptFormModal
        open={open}
        onClose={() => setOpen(false)}
        onSaved={(prompt) => console.log("Saved:", prompt)}
      />
    </>
  );
}

export const CreateNew: Story = {
  render: () => <CreateNewStory />,
};

function WithInitialContentStory() {
  const [open, setOpen] = useState(true);
  const initialContent =
    "You are a helpful coding assistant specializing in TypeScript and React. Always provide clear explanations and working examples.";

  return (
    <>
      <Button onClick={() => setOpen(true)}>Save Current Prompt</Button>
      <div className="mt-4 p-4 bg-muted rounded-md max-w-md">
        <p className="text-xs text-muted-foreground mb-2">Current system prompt:</p>
        <p className="text-sm">{initialContent}</p>
      </div>
      <PromptFormModal
        open={open}
        onClose={() => setOpen(false)}
        initialContent={initialContent}
        onSaved={(prompt) => console.log("Saved:", prompt)}
      />
    </>
  );
}

export const WithInitialContent: Story = {
  render: () => <WithInitialContentStory />,
};

function EditExistingStory() {
  const [open, setOpen] = useState(true);

  return (
    <>
      <Button onClick={() => setOpen(true)}>Edit Prompt</Button>
      <div className="mt-4 p-4 bg-muted rounded-md max-w-md">
        <p className="text-xs text-muted-foreground mb-1">Editing:</p>
        <p className="text-sm font-medium">{mockPrompt.name}</p>
        <p className="text-xs text-muted-foreground mt-1">{mockPrompt.description}</p>
      </div>
      <PromptFormModal
        open={open}
        onClose={() => setOpen(false)}
        editingPrompt={mockPrompt}
        onSaved={(prompt) => console.log("Updated:", prompt)}
      />
    </>
  );
}

export const EditExisting: Story = {
  render: () => <EditExistingStory />,
};

function LongContentStory() {
  const [open, setOpen] = useState(true);
  const longContent = `You are an advanced AI assistant with expertise in multiple domains.

## Core Responsibilities
1. Provide accurate and helpful information
2. Break down complex topics into understandable explanations
3. Offer practical solutions and actionable advice
4. Maintain a professional yet friendly tone

## Communication Guidelines
- Use clear, concise language
- Provide examples when helpful
- Ask clarifying questions when the request is ambiguous
- Acknowledge limitations when appropriate

## Technical Expertise
- Software development (multiple languages and frameworks)
- Data analysis and visualization
- System architecture and design patterns
- DevOps and cloud infrastructure

## Ethical Guidelines
- Never provide harmful or dangerous information
- Respect user privacy and data security
- Be transparent about AI limitations
- Encourage best practices and safe behaviors`;

  return (
    <>
      <Button onClick={() => setOpen(true)}>Save Long Prompt</Button>
      <PromptFormModal
        open={open}
        onClose={() => setOpen(false)}
        initialContent={longContent}
        onSaved={(prompt) => console.log("Saved:", prompt)}
      />
    </>
  );
}

export const LongContent: Story = {
  render: () => <LongContentStory />,
};
