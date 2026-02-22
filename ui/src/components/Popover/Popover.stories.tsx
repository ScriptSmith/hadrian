import type { Meta, StoryObj } from "@storybook/react";
import { Popover, PopoverTrigger, PopoverContent } from "./Popover";
import { Button } from "../Button/Button";

const meta: Meta<typeof Popover> = {
  title: "UI/Popover",
  component: Popover,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => (
    <Popover>
      <PopoverTrigger asChild>
        <Button>Open Popover</Button>
      </PopoverTrigger>
      <PopoverContent>
        <p>This is the popover content.</p>
      </PopoverContent>
    </Popover>
  ),
};

export const WithForm: Story = {
  render: () => (
    <Popover>
      <PopoverTrigger asChild>
        <Button>Edit Settings</Button>
      </PopoverTrigger>
      <PopoverContent className="w-80">
        <div className="space-y-4">
          <h3 className="font-medium">Settings</h3>
          <div className="space-y-2">
            <label htmlFor="story-name" className="text-sm text-muted-foreground">
              Name
            </label>
            <input
              id="story-name"
              type="text"
              className="w-full rounded border px-3 py-2 text-sm"
              placeholder="Enter name..."
            />
          </div>
          <Button size="sm" className="w-full">
            Save
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  ),
};

export const PlacementBottom: Story = {
  render: () => (
    <Popover>
      <PopoverTrigger asChild>
        <Button>Bottom Popover</Button>
      </PopoverTrigger>
      <PopoverContent side="bottom">
        <p>This popover appears below the trigger.</p>
      </PopoverContent>
    </Popover>
  ),
};

export const PlacementTop: Story = {
  render: () => (
    <div className="pt-32">
      <Popover>
        <PopoverTrigger asChild>
          <Button>Top Popover</Button>
        </PopoverTrigger>
        <PopoverContent side="top">
          <p>This popover appears above the trigger.</p>
        </PopoverContent>
      </Popover>
    </div>
  ),
};
