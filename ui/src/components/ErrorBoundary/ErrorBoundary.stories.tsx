import type { Meta, StoryObj } from "@storybook/react";
import type { ReactNode } from "react";
import { ErrorBoundary } from "./ErrorBoundary";

const meta: Meta<typeof ErrorBoundary> = {
  title: "UI/ErrorBoundary",
  component: ErrorBoundary,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

function ErrorThrowingComponent(): ReactNode {
  throw new Error("This is a simulated error for testing the ErrorBoundary component.");
}

export const WithError: Story = {
  render: () => (
    <ErrorBoundary>
      <ErrorThrowingComponent />
    </ErrorBoundary>
  ),
};

export const WithoutError: Story = {
  render: () => (
    <ErrorBoundary>
      <div className="p-4 text-center">
        <p>This content renders normally when there are no errors.</p>
      </div>
    </ErrorBoundary>
  ),
};
