import type { Meta, StoryObj } from "@storybook/react";
import { User, Bot } from "lucide-react";
import { Avatar, AvatarImage, AvatarFallback, getInitials } from "./Avatar";

const meta: Meta<typeof Avatar> = {
  title: "UI/Avatar",
  component: Avatar,
  parameters: {
    layout: "centered",
  },

  argTypes: {
    size: {
      control: "select",
      options: ["sm", "md", "lg"],
    },
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

export const WithFallback: Story = {
  render: () => (
    <Avatar>
      <AvatarFallback>JD</AvatarFallback>
    </Avatar>
  ),
};

export const WithImage: Story = {
  render: () => (
    <Avatar>
      <AvatarImage src="https://github.com/shadcn.png" alt="User avatar" />
      <AvatarFallback>CN</AvatarFallback>
    </Avatar>
  ),
};

export const WithIcon: Story = {
  render: () => (
    <Avatar>
      <AvatarFallback className="bg-primary text-primary-foreground">
        <User className="h-4 w-4" />
      </AvatarFallback>
    </Avatar>
  ),
};

export const BotAvatar: Story = {
  render: () => (
    <Avatar>
      <AvatarFallback className="bg-secondary">
        <Bot className="h-4 w-4" />
      </AvatarFallback>
    </Avatar>
  ),
};

export const Small: Story = {
  render: () => (
    <Avatar size="sm">
      <AvatarFallback>SM</AvatarFallback>
    </Avatar>
  ),
};

export const Medium: Story = {
  render: () => (
    <Avatar size="md">
      <AvatarFallback>MD</AvatarFallback>
    </Avatar>
  ),
};

export const Large: Story = {
  render: () => (
    <Avatar size="lg">
      <AvatarFallback>LG</AvatarFallback>
    </Avatar>
  ),
};

export const AllSizes: Story = {
  render: () => (
    <div className="flex items-center gap-4">
      <Avatar size="sm">
        <AvatarFallback>SM</AvatarFallback>
      </Avatar>
      <Avatar size="md">
        <AvatarFallback>MD</AvatarFallback>
      </Avatar>
      <Avatar size="lg">
        <AvatarFallback>LG</AvatarFallback>
      </Avatar>
    </div>
  ),
};

export const GetInitialsDemo: Story = {
  render: () => (
    <div className="flex flex-col gap-4">
      {["John Doe", "Alice", "Bob Smith Johnson", "Mary Jane Watson"].map((name) => (
        <div key={name} className="flex items-center gap-3">
          <Avatar>
            <AvatarFallback>{getInitials(name)}</AvatarFallback>
          </Avatar>
          <span>{name}</span>
        </div>
      ))}
    </div>
  ),
};

export const UserList: Story = {
  render: () => (
    <div className="space-y-3">
      {[
        { name: "Alice Johnson", role: "Admin" },
        { name: "Bob Smith", role: "Developer" },
        { name: "Charlie Brown", role: "Designer" },
      ].map((user) => (
        <div key={user.name} className="flex items-center gap-3">
          <Avatar>
            <AvatarFallback className="bg-primary text-primary-foreground">
              {getInitials(user.name)}
            </AvatarFallback>
          </Avatar>
          <div>
            <div className="font-medium">{user.name}</div>
            <div className="text-sm text-muted-foreground">{user.role}</div>
          </div>
        </div>
      ))}
    </div>
  ),
};
