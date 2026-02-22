import { useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { Meta, StoryObj } from "@storybook/react";
import { MemoryRouter } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { AuthProvider } from "@/auth";
import { ConfigProvider } from "@/config/ConfigProvider";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import { ConfirmDialogProvider } from "../ConfirmDialog/ConfirmDialog";
import { ConversationsProvider } from "../ConversationsProvider/ConversationsProvider";
import { ToastProvider } from "../Toast/Toast";
import { useResizable } from "@/hooks/useResizable";
import { SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH, SIDEBAR_DEFAULT_WIDTH } from "@/preferences/types";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
    },
  },
});

const meta: Meta<typeof Sidebar> = {
  title: "Layout/Sidebar",
  component: Sidebar,
  parameters: {
    layout: "fullscreen",
  },

  decorators: [
    (Story, context) => {
      const initialEntries = context.parameters.initialEntries || ["/chat"];
      return (
        <QueryClientProvider client={queryClient}>
          <MemoryRouter initialEntries={initialEntries}>
            <ConfigProvider>
              <AuthProvider>
                <PreferencesProvider>
                  <ToastProvider>
                    <ConfirmDialogProvider>
                      <ConversationsProvider>
                        <div style={{ height: "100vh", display: "flex" }}>
                          <Story />
                        </div>
                      </ConversationsProvider>
                    </ConfirmDialogProvider>
                  </ToastProvider>
                </PreferencesProvider>
              </AuthProvider>
            </ConfigProvider>
          </MemoryRouter>
        </QueryClientProvider>
      );
    },
  ],
  args: {
    onClose: () => {},
    onCollapsedChange: () => {},
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Open: Story = {
  args: {
    open: true,
    collapsed: false,
  },
};

export const Collapsed: Story = {
  args: {
    open: true,
    collapsed: true,
  },
};

/** Interactive resizable sidebar wrapper component */
function ResizableSidebarDemo() {
  const [width, setWidth] = useState(SIDEBAR_DEFAULT_WIDTH);

  const { isDragging, handleProps } = useResizable({
    initialWidth: width,
    minWidth: SIDEBAR_MIN_WIDTH,
    maxWidth: SIDEBAR_MAX_WIDTH,
    onResize: setWidth,
    onResizeEnd: setWidth,
  });

  return (
    <div style={{ display: "flex", height: "100vh", width: "100%" }}>
      <Sidebar
        open={true}
        collapsed={false}
        onClose={() => {}}
        onCollapsedChange={() => {}}
        width={width}
        isResizing={isDragging}
        resizeHandleProps={handleProps}
      />
      <div
        style={{
          flex: 1,
          padding: "1rem",
          borderLeft: "1px solid var(--border)",
          backgroundColor: "var(--background)",
        }}
      >
        <p style={{ color: "var(--muted-foreground)", fontSize: "0.875rem" }}>
          Drag the right edge of the sidebar to resize it. Current width:{" "}
          <strong>{Math.round(width)}px</strong>
        </p>
        <p
          style={{
            color: "var(--muted-foreground)",
            fontSize: "0.75rem",
            marginTop: "0.5rem",
          }}
        >
          Double-click the edge to reset to default width ({SIDEBAR_DEFAULT_WIDTH}px)
        </p>
      </div>
    </div>
  );
}

export const Resizable: Story = {
  render: () => <ResizableSidebarDemo />,
  parameters: {
    docs: {
      description: {
        story:
          "Sidebar with resizable functionality. Drag the right edge to resize between 180px and 400px. Double-click to reset to default width.",
      },
    },
  },
};

export const CustomWidth: Story = {
  args: {
    open: true,
    collapsed: false,
    width: 320,
  },
  parameters: {
    docs: {
      description: {
        story: "Sidebar with a custom width of 320px.",
      },
    },
  },
};

export const MinWidth: Story = {
  args: {
    open: true,
    collapsed: false,
    width: SIDEBAR_MIN_WIDTH,
  },
  parameters: {
    docs: {
      description: {
        story: `Sidebar at minimum width (${SIDEBAR_MIN_WIDTH}px).`,
      },
    },
  },
};

export const MaxWidth: Story = {
  args: {
    open: true,
    collapsed: false,
    width: SIDEBAR_MAX_WIDTH,
  },
  parameters: {
    docs: {
      description: {
        story: `Sidebar at maximum width (${SIDEBAR_MAX_WIDTH}px).`,
      },
    },
  },
};
