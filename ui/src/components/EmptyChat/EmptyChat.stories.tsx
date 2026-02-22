import type { Meta, StoryObj } from "@storybook/react";
import { expect, within, userEvent } from "storybook/test";

import { EmptyChat } from "./EmptyChat";

const meta: Meta<typeof EmptyChat> = {
  title: "Chat/EmptyChat",
  component: EmptyChat,
  parameters: {
    layout: "fullscreen",
  },
  decorators: [
    (Story) => (
      <div className="h-[600px] flex items-center justify-center">
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

export const NoModelSelected: Story = {
  args: {
    selectedModels: [],
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Should show prompt to select a model
    await expect(canvas.getByText(/select a model/i)).toBeInTheDocument();

    // Should still show example prompts even without model selected
    await expect(canvas.getByText("General")).toBeInTheDocument();
    await expect(canvas.getByText("Coding")).toBeInTheDocument();
    await expect(canvas.getByText("Research")).toBeInTheDocument();
  },
};

/**
 * Test: Category selection works without model selected
 */
export const NoModelWithCategorySelection: Story = {
  args: {
    selectedModels: [],
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Should show prompt to select a model
    await expect(canvas.getByText(/select a model/i)).toBeInTheDocument();

    // Click on Coding category
    const codingTab = canvas.getByRole("button", { name: /coding/i });
    await userEvent.click(codingTab);

    // Should show coding prompts even without model selected
    await expect(canvas.getByText("Debug this code")).toBeInTheDocument();
    await expect(canvas.getByText("Code review")).toBeInTheDocument();
  },
};

export const SingleModel: Story = {
  args: {
    selectedModels: ["anthropic/claude-3-opus"],
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Should show model name
    await expect(canvas.getByText(/claude-3-opus/i)).toBeInTheDocument();

    // Should show example prompt categories
    await expect(canvas.getByText("General")).toBeInTheDocument();
    await expect(canvas.getByText("Coding")).toBeInTheDocument();
    await expect(canvas.getByText("Research")).toBeInTheDocument();
    await expect(canvas.getByText("Data Analysis")).toBeInTheDocument();
    await expect(canvas.getByText("Writing")).toBeInTheDocument();
    await expect(canvas.getByText("Creative")).toBeInTheDocument();
  },
};

export const MultipleModels: Story = {
  args: {
    selectedModels: ["anthropic/claude-3-opus", "openai/gpt-4o", "google/gemini-1.5-pro"],
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Should show model count
    await expect(canvas.getByText(/3 models/i)).toBeInTheDocument();

    // Should show example prompt categories
    await expect(canvas.getByText("General")).toBeInTheDocument();
  },
};

export const LoadingModels: Story = {
  args: {
    selectedModels: [],
    isLoadingModels: true,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Should show loading message
    await expect(canvas.getByText(/loading available models/i)).toBeInTheDocument();

    // Should NOT show example prompts when loading
    await expect(canvas.queryByText(/General/i)).not.toBeInTheDocument();
  },
};

/**
 * Test: Clicking a category shows its prompts
 */
export const CategorySelection: Story = {
  args: {
    selectedModels: ["anthropic/claude-3-opus"],
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Initially should show hint
    await expect(canvas.getByText(/click a category/i)).toBeInTheDocument();

    // Click on Coding category
    const codingTab = canvas.getByRole("button", { name: /coding/i });
    await userEvent.click(codingTab);

    // Should show coding prompts
    await expect(canvas.getByText("Debug this code")).toBeInTheDocument();
    await expect(canvas.getByText("Code review")).toBeInTheDocument();
    await expect(canvas.getByText("Implement feature")).toBeInTheDocument();

    // Click again to deselect
    await userEvent.click(codingTab);

    // Should hide prompts and show hint again
    await expect(canvas.queryByText("Debug this code")).not.toBeInTheDocument();
    await expect(canvas.getByText(/click a category/i)).toBeInTheDocument();
  },
};

/**
 * Test: Switching between categories
 */
export const CategorySwitching: Story = {
  args: {
    selectedModels: ["openai/gpt-4o"],
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Click on General category
    const generalTab = canvas.getByRole("button", { name: /general/i });
    await userEvent.click(generalTab);

    // Should show general prompts
    await expect(canvas.getByText("Explain a concept")).toBeInTheDocument();

    // Switch to Writing category
    const writingTab = canvas.getByRole("button", { name: /writing/i });
    await userEvent.click(writingTab);

    // Should show writing prompts and hide general prompts
    await expect(canvas.getByText("Draft document")).toBeInTheDocument();
    await expect(canvas.queryByText("Explain a concept")).not.toBeInTheDocument();
  },
};

/**
 * Test: All categories display correctly
 */
export const AllCategories: Story = {
  args: {
    selectedModels: ["anthropic/claude-3-opus"],
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    const categories = ["General", "Coding", "Research", "Data Analysis", "Writing", "Creative"];

    for (const category of categories) {
      const tab = canvas.getByRole("button", { name: new RegExp(category, "i") });
      await userEvent.click(tab);

      // Each category should have at least one prompt visible
      const promptCards = canvas.getAllByRole("button").filter((btn) => {
        // Filter to only prompt cards (not category tabs)
        return btn.className.includes("rounded-lg");
      });

      // Should have prompt cards visible
      expect(promptCards.length).toBeGreaterThan(0);
    }
  },
};
