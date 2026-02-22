import { memo } from "react";
import { Code, Table, BarChart3, Image, FileCode, Bot, ExternalLink, Search } from "lucide-react";
import type { Artifact, ArtifactType } from "@/components/chat-types";

interface ArtifactThumbnailProps {
  artifact: Artifact;
  /** Click handler to expand the artifact */
  onClick?: () => void;
  /** Optional label showing origin (e.g., "from: Python") */
  originLabel?: string;
}

/** Icon mapping for artifact types */
const ARTIFACT_ICONS: Record<ArtifactType, typeof Code> = {
  code: Code,
  table: Table,
  chart: BarChart3,
  image: Image,
  html: FileCode,
  agent: Bot,
  file_search: Search,
  // display_selection is internal and shouldn't be rendered as a thumbnail
  display_selection: Code,
};

/** Display labels for artifact types */
const ARTIFACT_LABELS: Record<ArtifactType, string> = {
  code: "Code",
  table: "Table",
  chart: "Chart",
  image: "Image",
  html: "HTML",
  agent: "Sub-Agent",
  file_search: "Search Results",
  display_selection: "Selection",
};

/** Background colors for artifact type badges */
const ARTIFACT_COLORS: Record<ArtifactType, string> = {
  code: "bg-amber-500/10 text-amber-800 dark:text-amber-400",
  table: "bg-blue-500/10 text-blue-700 dark:text-blue-400",
  chart: "bg-purple-500/10 text-purple-700 dark:text-purple-400",
  image: "bg-green-500/10 text-green-800 dark:text-green-400",
  html: "bg-pink-500/10 text-pink-700 dark:text-pink-400",
  agent: "bg-cyan-500/10 text-cyan-800 dark:text-cyan-400",
  file_search: "bg-indigo-500/10 text-indigo-700 dark:text-indigo-400",
  display_selection: "bg-zinc-500/10 text-zinc-700 dark:text-zinc-400",
};

/**
 * Compact preview card for an artifact
 *
 * Shows a minimal representation with type icon and title.
 * Clicking expands to full view.
 */
function ArtifactThumbnailComponent({
  artifact,
  onClick,
  originLabel: _originLabel,
}: ArtifactThumbnailProps) {
  const Icon = ARTIFACT_ICONS[artifact.type] || Code;
  const label = artifact.title || ARTIFACT_LABELS[artifact.type];
  const colorClass = ARTIFACT_COLORS[artifact.type] || ARTIFACT_COLORS.code;

  return (
    <button
      type="button"
      onClick={onClick}
      className="group inline-flex items-center gap-1.5 rounded border border-zinc-200 bg-white px-2 py-1 text-left transition-colors hover:border-zinc-300 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-800 dark:hover:border-zinc-600 dark:hover:bg-zinc-750"
    >
      {/* Type icon */}
      <div className={`rounded p-0.5 ${colorClass}`}>
        <Icon className="h-3 w-3" />
      </div>

      {/* Title */}
      <span className="truncate text-[11px] font-medium text-zinc-700 dark:text-zinc-300 max-w-[120px]">
        {label}
      </span>

      {/* Expand indicator */}
      <ExternalLink className="h-2.5 w-2.5 text-zinc-400 opacity-0 transition-opacity group-hover:opacity-100 dark:text-zinc-500 shrink-0" />
    </button>
  );
}

export const ArtifactThumbnail = memo(ArtifactThumbnailComponent);
