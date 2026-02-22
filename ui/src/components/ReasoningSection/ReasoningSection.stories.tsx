import type { Meta, StoryObj } from "@storybook/react";

import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import { ReasoningSection } from "./ReasoningSection";

const meta: Meta<typeof ReasoningSection> = {
  title: "Components/ReasoningSection",
  component: ReasoningSection,
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
  argTypes: {
    content: { control: "text" },
    isStreaming: { control: "boolean" },
    tokenCount: { control: "number" },
  },
};

export default meta;
type Story = StoryObj<typeof ReasoningSection>;

const sampleReasoning = `Let me think through this step by step...

1. **Understanding the Problem**
   - The user is asking about implementing a binary search algorithm
   - This is a classic divide-and-conquer approach

2. **Key Considerations**
   - The array must be sorted for binary search to work
   - We need to handle edge cases like empty arrays
   - Time complexity should be O(log n)

3. **Implementation Strategy**
   - Use two pointers: left and right
   - Calculate mid point
   - Compare target with mid element
   - Adjust pointers based on comparison

Let me now provide the implementation...`;

export const Default: Story = {
  args: {
    content: sampleReasoning,
    isStreaming: false,
    tokenCount: 156,
  },
};

export const Streaming: Story = {
  args: {
    content: "Let me think through this step by step...\n\n1. **Understanding the Problem**",
    isStreaming: true,
    tokenCount: 42,
  },
};

export const Empty: Story = {
  args: {
    content: "",
    isStreaming: false,
  },
};

export const StreamingEmpty: Story = {
  args: {
    content: "",
    isStreaming: true,
  },
};

export const LongContent: Story = {
  args: {
    content: `${sampleReasoning}\n\n---\n\n${sampleReasoning}\n\n---\n\n${sampleReasoning}`,
    isStreaming: false,
    tokenCount: 468,
  },
};

export const WithCode: Story = {
  args: {
    content: `I'll analyze this code and explain my reasoning:

\`\`\`python
def binary_search(arr, target):
    left, right = 0, len(arr) - 1

    while left <= right:
        mid = (left + right) // 2
        if arr[mid] == target:
            return mid
        elif arr[mid] < target:
            left = mid + 1
        else:
            right = mid - 1

    return -1
\`\`\`

The key insight here is that we're halving the search space with each iteration.`,
    isStreaming: false,
    tokenCount: 234,
  },
};

export const NoTokenCount: Story = {
  args: {
    content: sampleReasoning,
    isStreaming: false,
  },
};
