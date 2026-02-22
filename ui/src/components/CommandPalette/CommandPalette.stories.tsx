import type { Meta, StoryObj } from "@storybook/react";
import { useEffect } from "react";
import { CommandPaletteProvider, useCommandPalette } from "./CommandPalette";
import { Button } from "../Button/Button";
import { Settings, Users, FileText, Search } from "lucide-react";

const meta: Meta = {
  title: "UI/CommandPalette",
  parameters: {
    layout: "centered",
  },

  decorators: [
    (Story) => (
      <CommandPaletteProvider>
        <Story />
      </CommandPaletteProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

function CommandPaletteDemo() {
  const { setOpen, registerCommand, unregisterCommand } = useCommandPalette();

  useEffect(() => {
    const commands = [
      {
        id: "settings",
        label: "Open Settings",
        description: "View and edit your settings",
        icon: <Settings className="h-4 w-4" />,
        shortcut: ["⌘", "S"],
        onSelect: () => console.log("Settings selected"),
        category: "Navigation",
      },
      {
        id: "users",
        label: "Manage Users",
        description: "Add or remove team members",
        icon: <Users className="h-4 w-4" />,
        onSelect: () => console.log("Users selected"),
        category: "Navigation",
      },
      {
        id: "docs",
        label: "View Documentation",
        description: "Read the documentation",
        icon: <FileText className="h-4 w-4" />,
        onSelect: () => console.log("Docs selected"),
        category: "Help",
      },
      {
        id: "search",
        label: "Search",
        description: "Search across all resources",
        icon: <Search className="h-4 w-4" />,
        shortcut: ["⌘", "F"],
        onSelect: () => console.log("Search selected"),
        category: "Actions",
      },
    ];

    commands.forEach(registerCommand);
    return () => commands.forEach((c) => unregisterCommand(c.id));
  }, [registerCommand, unregisterCommand]);

  return (
    <div className="space-y-4 text-center">
      <p className="text-sm text-muted-foreground">
        Press <kbd className="rounded bg-muted px-1.5 py-0.5 font-mono">⌘K</kbd> or click the button
      </p>
      <Button onClick={() => setOpen(true)}>Open Command Palette</Button>
    </div>
  );
}

export const Default: Story = {
  render: () => <CommandPaletteDemo />,
};
