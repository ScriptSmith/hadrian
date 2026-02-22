import type { Meta, StoryObj } from "@storybook/react";
import {
  PythonIcon,
  JavaScriptIcon,
  SqlIcon,
  TOOL_ICON_MAP,
  TOOL_SHORT_NAMES,
  getToolIcon,
} from "./ToolIcons";

const meta = {
  title: "Components/ToolIcons",
  parameters: {
    layout: "centered",
  },
} satisfies Meta;

export default meta;

/** All tool icons in a grid */
export const AllIcons: StoryObj = {
  render: () => (
    <div className="space-y-6">
      <div className="grid grid-cols-3 gap-4">
        {Object.entries(TOOL_ICON_MAP).map(([toolId, Icon]) => (
          <div key={toolId} className="flex flex-col items-center gap-2 rounded-lg border p-4">
            <Icon className="h-8 w-8" />
            <span className="text-xs font-medium">{TOOL_SHORT_NAMES[toolId] || toolId}</span>
            <span className="text-[10px] text-muted-foreground">{toolId}</span>
          </div>
        ))}
      </div>
    </div>
  ),
};

/** Python icon at different sizes */
export const PythonIconSizes: StoryObj = {
  render: () => (
    <div className="flex items-end gap-4">
      <div className="flex flex-col items-center gap-1">
        <PythonIcon className="h-3 w-3" />
        <span className="text-[10px]">12px</span>
      </div>
      <div className="flex flex-col items-center gap-1">
        <PythonIcon className="h-4 w-4" />
        <span className="text-[10px]">16px</span>
      </div>
      <div className="flex flex-col items-center gap-1">
        <PythonIcon className="h-5 w-5" />
        <span className="text-[10px]">20px</span>
      </div>
      <div className="flex flex-col items-center gap-1">
        <PythonIcon className="h-6 w-6" />
        <span className="text-[10px]">24px</span>
      </div>
      <div className="flex flex-col items-center gap-1">
        <PythonIcon className="h-8 w-8" />
        <span className="text-[10px]">32px</span>
      </div>
    </div>
  ),
};

/** JavaScript icon at different sizes */
export const JavaScriptIconSizes: StoryObj = {
  render: () => (
    <div className="flex items-end gap-4">
      <div className="flex flex-col items-center gap-1">
        <JavaScriptIcon className="h-3 w-3" />
        <span className="text-[10px]">12px</span>
      </div>
      <div className="flex flex-col items-center gap-1">
        <JavaScriptIcon className="h-4 w-4" />
        <span className="text-[10px]">16px</span>
      </div>
      <div className="flex flex-col items-center gap-1">
        <JavaScriptIcon className="h-5 w-5" />
        <span className="text-[10px]">20px</span>
      </div>
      <div className="flex flex-col items-center gap-1">
        <JavaScriptIcon className="h-6 w-6" />
        <span className="text-[10px]">24px</span>
      </div>
      <div className="flex flex-col items-center gap-1">
        <JavaScriptIcon className="h-8 w-8" />
        <span className="text-[10px]">32px</span>
      </div>
    </div>
  ),
};

/** SQL icon at different sizes */
export const SqlIconSizes: StoryObj = {
  render: () => (
    <div className="flex items-end gap-4">
      <div className="flex flex-col items-center gap-1">
        <SqlIcon className="h-3 w-3" />
        <span className="text-[10px]">12px</span>
      </div>
      <div className="flex flex-col items-center gap-1">
        <SqlIcon className="h-4 w-4" />
        <span className="text-[10px]">16px</span>
      </div>
      <div className="flex flex-col items-center gap-1">
        <SqlIcon className="h-5 w-5" />
        <span className="text-[10px]">20px</span>
      </div>
      <div className="flex flex-col items-center gap-1">
        <SqlIcon className="h-6 w-6" />
        <span className="text-[10px]">24px</span>
      </div>
      <div className="flex flex-col items-center gap-1">
        <SqlIcon className="h-8 w-8" />
        <span className="text-[10px]">32px</span>
      </div>
    </div>
  ),
};

/** Icons with colors */
export const ColoredIcons: StoryObj = {
  render: () => (
    <div className="flex items-center gap-4">
      <PythonIcon className="h-6 w-6 text-orange-500" />
      <JavaScriptIcon className="h-6 w-6 text-yellow-500" />
      <SqlIcon className="h-6 w-6 text-cyan-500" />
      {(() => {
        const ChartIcon = getToolIcon("chart_render");
        return <ChartIcon className="h-6 w-6 text-emerald-500" />;
      })()}
      {(() => {
        const SearchIcon = getToolIcon("file_search");
        return <SearchIcon className="h-6 w-6 text-blue-500" />;
      })()}
    </div>
  ),
};

/** Tool badges as they appear in the UI */
export const ToolBadges: StoryObj = {
  render: () => (
    <div className="flex flex-wrap gap-2">
      {Object.entries(TOOL_ICON_MAP).map(([toolId, Icon]) => (
        <span
          key={toolId}
          className="flex items-center gap-1 rounded bg-zinc-200/50 px-1.5 py-0.5 text-xs text-zinc-600 dark:bg-zinc-700/50 dark:text-zinc-400"
        >
          <Icon className="h-3 w-3" />
          <span>{TOOL_SHORT_NAMES[toolId] || toolId}</span>
        </span>
      ))}
    </div>
  ),
};
