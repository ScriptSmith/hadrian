import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";
import { Pencil, Trash2, Pin, Copy, ExternalLink } from "lucide-react";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator } from "./ContextMenu";

const meta: Meta<typeof ContextMenu> = {
  title: "Components/ContextMenu",
  component: ContextMenu,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof ContextMenu>;

function ContextMenuDemo() {
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState<{ x: number; y: number } | null>(null);
  const [lastAction, setLastAction] = useState<string>("");

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    setPosition({ x: e.clientX, y: e.clientY });
    setOpen(true);
  };

  return (
    <div className="flex flex-col items-center gap-4">
      <div
        className="flex h-40 w-64 items-center justify-center rounded-lg border-2 border-dashed border-muted-foreground/30 bg-muted/20 text-sm text-muted-foreground"
        onContextMenu={handleContextMenu}
      >
        Right-click here
      </div>

      {lastAction && (
        <div className="text-sm text-muted-foreground">
          Last action: <span className="font-medium text-foreground">{lastAction}</span>
        </div>
      )}

      <ContextMenu open={open} onOpenChange={setOpen} position={position}>
        <ContextMenuItem onClick={() => setLastAction("Pin")}>
          <Pin className="mr-2 h-4 w-4" />
          Pin
        </ContextMenuItem>
        <ContextMenuItem onClick={() => setLastAction("Rename")}>
          <Pencil className="mr-2 h-4 w-4" />
          Rename
        </ContextMenuItem>
        <ContextMenuItem onClick={() => setLastAction("Copy Link")}>
          <Copy className="mr-2 h-4 w-4" />
          Copy Link
        </ContextMenuItem>
        <ContextMenuItem onClick={() => setLastAction("Open in New Tab")}>
          <ExternalLink className="mr-2 h-4 w-4" />
          Open in New Tab
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem className="text-destructive" onClick={() => setLastAction("Delete")}>
          <Trash2 className="mr-2 h-4 w-4" />
          Delete
        </ContextMenuItem>
      </ContextMenu>
    </div>
  );
}

export const Default: Story = {
  render: () => <ContextMenuDemo />,
};

function MultipleTargetsDemo() {
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState<{ x: number; y: number } | null>(null);
  const [selectedItem, setSelectedItem] = useState<string>("");
  const [lastAction, setLastAction] = useState<string>("");

  const items = ["Item 1", "Item 2", "Item 3", "Item 4"];

  const handleContextMenu = (e: React.MouseEvent, item: string) => {
    e.preventDefault();
    setSelectedItem(item);
    setPosition({ x: e.clientX, y: e.clientY });
    setOpen(true);
  };

  return (
    <div className="flex flex-col items-center gap-4">
      <div className="flex flex-col gap-2">
        {items.map((item) => (
          <div
            key={item}
            className="flex h-10 w-48 items-center rounded-lg bg-muted px-4 text-sm hover:bg-accent"
            onContextMenu={(e) => handleContextMenu(e, item)}
          >
            {item}
          </div>
        ))}
      </div>

      {lastAction && (
        <div className="text-sm text-muted-foreground">
          Last action: <span className="font-medium text-foreground">{lastAction}</span> on{" "}
          <span className="font-medium text-foreground">{selectedItem}</span>
        </div>
      )}

      <ContextMenu open={open} onOpenChange={setOpen} position={position}>
        <ContextMenuItem onClick={() => setLastAction("Edit")}>
          <Pencil className="mr-2 h-4 w-4" />
          Edit {selectedItem}
        </ContextMenuItem>
        <ContextMenuItem onClick={() => setLastAction("Duplicate")}>
          <Copy className="mr-2 h-4 w-4" />
          Duplicate
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem className="text-destructive" onClick={() => setLastAction("Delete")}>
          <Trash2 className="mr-2 h-4 w-4" />
          Delete
        </ContextMenuItem>
      </ContextMenu>
    </div>
  );
}

export const MultipleTargets: Story = {
  render: () => <MultipleTargetsDemo />,
};

function EdgePositioningDemo() {
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState<{ x: number; y: number } | null>(null);

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    setPosition({ x: e.clientX, y: e.clientY });
    setOpen(true);
  };

  return (
    <div className="h-[400px] w-[600px] overflow-hidden rounded-lg border">
      <div
        className="flex h-full w-full items-center justify-center bg-muted/20 text-sm text-muted-foreground"
        onContextMenu={handleContextMenu}
      >
        Right-click anywhere to test edge positioning
      </div>

      <ContextMenu open={open} onOpenChange={setOpen} position={position}>
        <ContextMenuItem onClick={() => {}}>Option 1</ContextMenuItem>
        <ContextMenuItem onClick={() => {}}>Option 2</ContextMenuItem>
        <ContextMenuItem onClick={() => {}}>Option 3</ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem onClick={() => {}}>Option 4</ContextMenuItem>
        <ContextMenuItem onClick={() => {}}>Option 5</ContextMenuItem>
      </ContextMenu>
    </div>
  );
}

export const EdgePositioning: Story = {
  render: () => <EdgePositioningDemo />,
  parameters: {
    docs: {
      description: {
        story:
          "Try right-clicking near the edges of the container. The menu will automatically adjust its position to stay within the viewport.",
      },
    },
  },
};
