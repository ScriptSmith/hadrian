import type { Meta, StoryObj } from "@storybook/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import SettingsPage from "./SettingsPage";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";

const meta: Meta<typeof SettingsPage> = {
  title: "Admin/SettingsPage",
  component: SettingsPage,
  parameters: {
    layout: "fullscreen",
    a11y: {
      config: {
        rules: [{ id: "heading-order", enabled: false }],
      },
    },
  },
  decorators: [
    (Story) => (
      <PreferencesProvider>
        <MemoryRouter initialEntries={["/admin/settings"]}>
          <Routes>
            <Route path="/admin/settings" element={<Story />} />
          </Routes>
        </MemoryRouter>
      </PreferencesProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const DarkMode: Story = {
  decorators: [
    (Story) => (
      <div className="dark bg-background text-foreground min-h-screen">
        <Story />
      </div>
    ),
  ],
};
