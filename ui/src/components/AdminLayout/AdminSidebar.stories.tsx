import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react";
import { MemoryRouter } from "react-router-dom";
import { AdminSidebar } from "./AdminSidebar";
import { useResizable } from "@/hooks/useResizable";
import { SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH } from "@/preferences/types";

const meta: Meta<typeof AdminSidebar> = {
  title: "Layout/AdminSidebar",
  component: AdminSidebar,
  parameters: {
    layout: "fullscreen",
  },
  decorators: [
    (Story, context) => {
      const initialEntries = context.parameters.initialEntries || ["/admin"];
      return (
        <MemoryRouter initialEntries={initialEntries}>
          <div style={{ height: "100vh", display: "flex" }}>
            <Story />
          </div>
        </MemoryRouter>
      );
    },
  ],
  args: {
    onCollapsedChange: () => {},
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  args: {
    collapsed: false,
    width: 220,
  },
};

export const Collapsed: Story = {
  args: {
    collapsed: true,
  },
};

export const OnDashboard: Story = {
  parameters: {
    initialEntries: ["/admin"],
  },
  args: {
    collapsed: false,
    width: 220,
  },
};

export const OnOrganizations: Story = {
  parameters: {
    initialEntries: ["/admin/organizations"],
  },
  args: {
    collapsed: false,
    width: 220,
  },
};

export const OnUsers: Story = {
  parameters: {
    initialEntries: ["/admin/users"],
  },
  args: {
    collapsed: false,
    width: 220,
  },
};

export const OnSSO: Story = {
  parameters: {
    initialEntries: ["/admin/sso"],
  },
  args: {
    collapsed: false,
    width: 220,
  },
};

export const OnSettings: Story = {
  parameters: {
    initialEntries: ["/admin/settings"],
  },
  args: {
    collapsed: false,
    width: 220,
  },
};

/** Interactive resizable sidebar */
function ResizableSidebarDemo() {
  const [width, setWidth] = useState(220);

  const { isDragging, handleProps } = useResizable({
    initialWidth: width,
    minWidth: SIDEBAR_MIN_WIDTH,
    maxWidth: SIDEBAR_MAX_WIDTH,
    onResize: setWidth,
    onResizeEnd: setWidth,
  });

  return (
    <AdminSidebar
      collapsed={false}
      width={width}
      isResizing={isDragging}
      resizeHandleProps={handleProps}
    />
  );
}

export const Resizable: Story = {
  render: () => <ResizableSidebarDemo />,
};

export const MinWidth: Story = {
  args: {
    collapsed: false,
    width: SIDEBAR_MIN_WIDTH,
  },
};

export const MaxWidth: Story = {
  args: {
    collapsed: false,
    width: SIDEBAR_MAX_WIDTH,
  },
};

/** Interactive collapsible sidebar */
function CollapsibleSidebarDemo() {
  const [collapsed, setCollapsed] = useState(false);

  return <AdminSidebar collapsed={collapsed} onCollapsedChange={setCollapsed} width={220} />;
}

export const Interactive: Story = {
  render: () => <CollapsibleSidebarDemo />,
};
