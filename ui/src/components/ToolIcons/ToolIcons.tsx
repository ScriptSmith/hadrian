/**
 * ToolIcons - Custom icons for tool types
 *
 * These icons are used across ToolsMenu, ToolCallIndicator, ExecutionSummaryBar,
 * and ArtifactThumbnail for consistent tool identification.
 */

import { FileSearch, Globe, BarChart3, Wrench, Bot, Plug } from "lucide-react";
import type { LucideIcon } from "lucide-react";

/** Python icon - "Py" text in a rounded box */
export function PythonIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="none">
      <rect
        x="2"
        y="2"
        width="20"
        height="20"
        rx="4"
        ry="4"
        stroke="currentColor"
        strokeWidth="1.5"
      />
      <text
        x="12"
        y="16"
        textAnchor="middle"
        fontSize="11"
        fontWeight="700"
        fontFamily="system-ui, sans-serif"
        fill="currentColor"
      >
        Py
      </text>
    </svg>
  );
}

/** JavaScript icon - "JS" text in a rounded box */
export function JavaScriptIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="none">
      <rect
        x="2"
        y="2"
        width="20"
        height="20"
        rx="4"
        ry="4"
        stroke="currentColor"
        strokeWidth="1.5"
      />
      <text
        x="12"
        y="16"
        textAnchor="middle"
        fontSize="11"
        fontWeight="700"
        fontFamily="system-ui, sans-serif"
        fill="currentColor"
      >
        JS
      </text>
    </svg>
  );
}

/** SQL icon - "SQL" text in a rounded box */
export function SqlIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="none">
      <rect
        x="2"
        y="2"
        width="20"
        height="20"
        rx="4"
        ry="4"
        stroke="currentColor"
        strokeWidth="1.5"
      />
      <text
        x="12"
        y="16"
        textAnchor="middle"
        fontSize="9"
        fontWeight="700"
        fontFamily="system-ui, sans-serif"
        fill="currentColor"
      >
        SQL
      </text>
    </svg>
  );
}

/** HTML icon - "</>" in a rounded box */
export function HtmlIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="none">
      <rect
        x="2"
        y="2"
        width="20"
        height="20"
        rx="4"
        ry="4"
        stroke="currentColor"
        strokeWidth="1.5"
      />
      <text
        x="12"
        y="16"
        textAnchor="middle"
        fontSize="10"
        fontWeight="700"
        fontFamily="system-ui, sans-serif"
        fill="currentColor"
      >
        {"</>"}
      </text>
    </svg>
  );
}

/** Wikipedia icon - "W" in a rounded box */
export function WikipediaIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="none">
      <rect
        x="2"
        y="2"
        width="20"
        height="20"
        rx="4"
        ry="4"
        stroke="currentColor"
        strokeWidth="1.5"
      />
      <text
        x="12"
        y="16.5"
        textAnchor="middle"
        fontSize="13"
        fontWeight="700"
        fontFamily="serif"
        fill="currentColor"
      >
        W
      </text>
    </svg>
  );
}

/** Wikidata icon - "Wd" in a rounded box */
export function WikidataIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="none">
      <rect
        x="2"
        y="2"
        width="20"
        height="20"
        rx="4"
        ry="4"
        stroke="currentColor"
        strokeWidth="1.5"
      />
      <text
        x="12"
        y="16"
        textAnchor="middle"
        fontSize="10"
        fontWeight="700"
        fontFamily="system-ui, sans-serif"
        fill="currentColor"
      >
        Wd
      </text>
    </svg>
  );
}

/** Icon component type (either Lucide or custom) */
export type ToolIconComponent = LucideIcon | React.FC<{ className?: string }>;

/** Tool ID to icon mapping - single source of truth */
export const TOOL_ICON_MAP: Record<string, ToolIconComponent> = {
  file_search: FileSearch,
  code_interpreter: PythonIcon,
  js_code_interpreter: JavaScriptIcon,
  sql_query: SqlIcon,
  chart_render: BarChart3,
  html_render: HtmlIcon,
  web_search: Globe,
  wikipedia: WikipediaIcon,
  wikidata: WikidataIcon,
  sub_agent: Bot,
  mcp: Plug,
  display_artifacts: Wrench,
};

/** Get icon for a tool ID */
export function getToolIcon(toolId: string): ToolIconComponent {
  return TOOL_ICON_MAP[toolId] || Wrench;
}

/** Tool short names for display */
export const TOOL_SHORT_NAMES: Record<string, string> = {
  file_search: "Search",
  code_interpreter: "Python",
  js_code_interpreter: "JS",
  sql_query: "SQL",
  chart_render: "Chart",
  html_render: "HTML",
  web_search: "Web",
  wikipedia: "Wikipedia",
  wikidata: "Wikidata",
  sub_agent: "Agent",
  mcp: "MCP",
  display_artifacts: "Display",
};

/** Get short display name for a tool */
export function getToolShortName(toolId: string): string {
  return TOOL_SHORT_NAMES[toolId] || toolId;
}
