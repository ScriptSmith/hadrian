import type { Meta, StoryObj } from "@storybook/react";

import { HierarchicalProgress } from "./HierarchicalProgress";
import type {
  HierarchicalSubtaskData,
  HierarchicalWorkerResultData,
} from "@/components/chat-types";

const meta = {
  title: "Components/HierarchicalProgress",
  component: HierarchicalProgress,
  parameters: {},
  decorators: [
    (Story) => (
      <div className="w-[600px]">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof HierarchicalProgress>;

export default meta;
type Story = StoryObj<typeof meta>;

const mockSubtasks: HierarchicalSubtaskData[] = [
  {
    id: "research",
    description: "Research the technical requirements",
    assignedModel: "gpt-4o",
    status: "complete",
  },
  {
    id: "design",
    description: "Design the solution",
    assignedModel: "claude-sonnet-4",
    status: "complete",
  },
  {
    id: "testing",
    description: "Plan testing strategy",
    assignedModel: "gemini-2.0-flash",
    status: "complete",
  },
];

const mockWorkerResults: HierarchicalWorkerResultData[] = [
  {
    subtaskId: "research",
    model: "gpt-4o",
    description: "Research the technical requirements",
    content:
      "Based on extensive research, the key technical requirements are:\n\n1. **Scalability**: The system must handle 10,000+ concurrent users\n2. **Performance**: Response times under 200ms for 95th percentile\n3. **Security**: SOC2 compliance required\n4. **Reliability**: 99.9% uptime SLA\n\nI recommend using a cloud-native architecture with Kubernetes for orchestration.",
    usage: { inputTokens: 500, outputTokens: 400, totalTokens: 900, cost: 0.0045 },
  },
  {
    subtaskId: "design",
    model: "claude-sonnet-4",
    description: "Design the solution",
    content:
      "The recommended solution architecture follows microservices patterns:\n\n**Core Components:**\n- API Gateway (Kong/Traefik)\n- Service Mesh (Istio)\n- Message Queue (RabbitMQ/Kafka)\n- Cache Layer (Redis Cluster)\n- Database (PostgreSQL with read replicas)\n\n**Key Design Decisions:**\n- Event-driven architecture for loose coupling\n- CQRS pattern for read/write optimization\n- Circuit breaker pattern for resilience",
    usage: { inputTokens: 600, outputTokens: 500, totalTokens: 1100, cost: 0.006 },
  },
  {
    subtaskId: "testing",
    model: "gemini-2.0-flash",
    description: "Plan testing strategy",
    content:
      "Testing strategy should include:\n\n1. **Unit Tests**: 80%+ coverage for business logic\n2. **Integration Tests**: API contract testing\n3. **E2E Tests**: Critical user journeys\n4. **Load Tests**: k6/Locust for performance validation\n5. **Chaos Engineering**: Gremlin for resilience testing",
    usage: { inputTokens: 400, outputTokens: 300, totalTokens: 700, cost: 0.002 },
  },
];

/**
 * Done state showing completed hierarchical delegation.
 * This is the typical view for persisted messages in the chat history.
 */
export const Done: Story = {
  args: {
    persistedMetadata: {
      subtasks: mockSubtasks,
      workerResults: mockWorkerResults,
      coordinatorModel: "claude-opus-4",
      aggregateUsage: {
        inputTokens: 1500,
        outputTokens: 1200,
        totalTokens: 2700,
        cost: 0.0125,
      },
    },
  },
};

/**
 * Many subtasks - testing with multiple worker results
 */
export const ManySubtasks: Story = {
  args: {
    persistedMetadata: {
      subtasks: [
        { id: "task-1", description: "First task", assignedModel: "gpt-4o", status: "complete" },
        {
          id: "task-2",
          description: "Second task",
          assignedModel: "claude-sonnet-4",
          status: "complete",
        },
        {
          id: "task-3",
          description: "Third task",
          assignedModel: "gemini-2.0-flash",
          status: "complete",
        },
        { id: "task-4", description: "Fourth task", assignedModel: "gpt-4o", status: "complete" },
        {
          id: "task-5",
          description: "Fifth task",
          assignedModel: "claude-sonnet-4",
          status: "complete",
        },
        {
          id: "task-6",
          description: "Sixth task",
          assignedModel: "gemini-2.0-flash",
          status: "complete",
        },
      ],
      workerResults: [
        {
          subtaskId: "task-1",
          model: "gpt-4o",
          description: "First task",
          content: "Completed first task with detailed analysis...",
          usage: { inputTokens: 200, outputTokens: 150, totalTokens: 350, cost: 0.002 },
        },
        {
          subtaskId: "task-2",
          model: "claude-sonnet-4",
          description: "Second task",
          content: "Completed second task with recommendations...",
          usage: { inputTokens: 250, outputTokens: 180, totalTokens: 430, cost: 0.002 },
        },
        {
          subtaskId: "task-3",
          model: "gemini-2.0-flash",
          description: "Third task",
          content: "Completed third task with implementation details...",
          usage: { inputTokens: 180, outputTokens: 120, totalTokens: 300, cost: 0.001 },
        },
        {
          subtaskId: "task-4",
          model: "gpt-4o",
          description: "Fourth task",
          content: "Completed fourth task...",
          usage: { inputTokens: 200, outputTokens: 150, totalTokens: 350, cost: 0.002 },
        },
        {
          subtaskId: "task-5",
          model: "claude-sonnet-4",
          description: "Fifth task",
          content: "Completed fifth task...",
          usage: { inputTokens: 220, outputTokens: 160, totalTokens: 380, cost: 0.002 },
        },
        {
          subtaskId: "task-6",
          model: "gemini-2.0-flash",
          description: "Sixth task",
          content: "Completed sixth task...",
          usage: { inputTokens: 150, outputTokens: 100, totalTokens: 250, cost: 0.001 },
        },
      ],
      coordinatorModel: "claude-opus-4",
      aggregateUsage: {
        inputTokens: 1200,
        outputTokens: 860,
        totalTokens: 2060,
        cost: 0.01,
      },
    },
  },
};

/**
 * Single subtask - simple hierarchical delegation
 */
export const SingleSubtask: Story = {
  args: {
    persistedMetadata: {
      subtasks: [
        {
          id: "research",
          description: "Research the topic thoroughly",
          assignedModel: "gpt-4o",
          status: "complete",
        },
      ],
      workerResults: [
        {
          subtaskId: "research",
          model: "gpt-4o",
          description: "Research the topic thoroughly",
          content: "Based on comprehensive research, here are the key findings...",
          usage: { inputTokens: 400, outputTokens: 300, totalTokens: 700, cost: 0.003 },
        },
      ],
      coordinatorModel: "claude-opus-4",
      aggregateUsage: {
        inputTokens: 600,
        outputTokens: 400,
        totalTokens: 1000,
        cost: 0.005,
      },
    },
  },
};
