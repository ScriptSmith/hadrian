import type { Meta, StoryObj } from "@storybook/react";
import { MemoryRouter } from "react-router-dom";
import { AppRoutes } from "./AppRoutes";

const meta: Meta<typeof AppRoutes> = {
  title: "Routes/AppRoutes",
  component: AppRoutes,
  decorators: [
    (Story) => (
      <MemoryRouter initialEntries={["/login"]}>
        <Story />
      </MemoryRouter>
    ),
  ],
  parameters: {
    layout: "fullscreen",
    a11y: {
      config: {
        rules: [
          // Route tree renders lazily — landmark/heading checks are irrelevant in isolation
          { id: "region", enabled: false },
          { id: "page-has-heading-one", enabled: false },
        ],
      },
    },
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

export const LoginRoute: Story = {};
