/**
 * ChartArtifact - Vega-Lite Chart Renderer
 *
 * Renders Vega-Lite specifications as interactive charts using vega-embed.
 * Supports all Vega-Lite chart types including bar, line, scatter, area, etc.
 */

import { memo, useState, useRef, useEffect } from "react";
import { Copy, Check, AlertCircle, Loader2 } from "lucide-react";
import embed, { type VisualizationSpec } from "vega-embed";

import type { Artifact, ChartArtifactData } from "@/components/chat-types";
import { Button } from "@/components/Button/Button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { cn } from "@/utils/cn";

export interface ChartArtifactProps {
  artifact: Artifact;
  className?: string;
}

function isChartArtifactData(data: unknown): data is ChartArtifactData {
  return (
    typeof data === "object" &&
    data !== null &&
    "spec" in data &&
    typeof (data as ChartArtifactData).spec === "object"
  );
}

function ChartArtifactComponent({ artifact, className }: ChartArtifactProps) {
  const [copied, setCopied] = useState(false);
  const [showSpec, setShowSpec] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const containerRef = useRef<HTMLDivElement>(null);

  // Validate and extract data
  const chartData = isChartArtifactData(artifact.data) ? artifact.data : null;
  const spec = chartData?.spec ?? null;
  const specJson = spec ? JSON.stringify(spec, null, 2) : "";

  // Render chart using vega-embed
  useEffect(() => {
    const container = containerRef.current;
    if (!container || !spec) {
      setIsLoading(false);
      return;
    }

    setIsLoading(true);
    setError(null);

    // Use vega-embed to render the chart
    embed(container, spec as VisualizationSpec, {
      // Responsive width
      width: container.clientWidth - 40,
      // Enable actions menu (export, view source, etc.)
      actions: {
        export: true,
        source: false, // We have our own source viewer
        compiled: false,
        editor: true,
      },
      // Use a theme that works well in both light and dark modes
      config: {
        background: "transparent",
        axis: {
          labelColor: "currentColor",
          titleColor: "currentColor",
          tickColor: "currentColor",
          domainColor: "currentColor",
          gridColor: "#e5e7eb",
        },
        legend: {
          labelColor: "currentColor",
          titleColor: "currentColor",
        },
        title: {
          color: "currentColor",
        },
      },
    })
      .then(() => {
        // Vega-embed renders a <details>/<summary> action menu with an SVG-only
        // <summary> that lacks an accessible name. Patch it post-render.
        const summary = container.querySelector("summary");
        if (summary && !summary.getAttribute("aria-label")) {
          summary.setAttribute("aria-label", "Chart actions");
        }

        // Vega-embed renders <g role="graphics-symbol"> elements without accessible
        // names. Add aria-label from their aria-roledescription to satisfy svg-img-alt.
        const symbols = container.querySelectorAll('[role="graphics-symbol"]');
        for (const el of symbols) {
          const desc = el.getAttribute("aria-roledescription");
          if (desc && !el.getAttribute("aria-label")) {
            el.setAttribute("aria-label", desc);
          }
        }

        setIsLoading(false);
      })
      .catch((err) => {
        console.error("Failed to render Vega-Lite chart:", err);
        setError(err instanceof Error ? err.message : "Failed to render chart");
        setIsLoading(false);
      });

    // Cleanup on unmount
    return () => {
      container.innerHTML = "";
    };
  }, [spec]);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(specJson);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  if (!chartData) {
    return <div className="p-4 text-sm text-muted-foreground">Invalid chart artifact data</div>;
  }

  return (
    <div className={cn("", className)}>
      {/* Chart container */}
      <div className="relative min-h-[200px]">
        {isLoading && (
          <div className="absolute inset-0 flex items-center justify-center bg-background/50">
            <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
          </div>
        )}

        {error ? (
          <div className="flex flex-col items-center justify-center py-8 px-4 text-center">
            <div className="h-12 w-12 rounded-full bg-destructive/10 flex items-center justify-center mb-3">
              <AlertCircle className="h-6 w-6 text-destructive" />
            </div>
            <p className="text-sm font-medium mb-1">Failed to render chart</p>
            <p className="text-xs text-muted-foreground max-w-xs">{error}</p>
          </div>
        ) : (
          <div ref={containerRef} className="p-4 flex justify-center" />
        )}
      </div>

      {/* Action buttons */}
      <div className="flex items-center justify-end gap-2 px-3 py-2 border-t bg-muted/30">
        <Button variant="ghost" size="sm" onClick={() => setShowSpec(!showSpec)}>
          {showSpec ? "Hide Spec" : "View Spec"}
        </Button>
      </div>

      {/* Spec JSON (collapsible) */}
      {showSpec && (
        <div className="relative border-t">
          <div className="absolute right-2 top-2 z-10">
            <Tooltip>
              <TooltipTrigger asChild>
                <Button variant="secondary" size="sm" className="h-7 w-7 p-0" onClick={handleCopy}>
                  {copied ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
                </Button>
              </TooltipTrigger>
              <TooltipContent>{copied ? "Copied!" : "Copy spec"}</TooltipContent>
            </Tooltip>
          </div>

          <pre className="p-4 overflow-x-auto text-xs font-mono text-muted-foreground max-h-[300px] overflow-y-auto">
            {specJson}
          </pre>
        </div>
      )}
    </div>
  );
}

export const ChartArtifact = memo(ChartArtifactComponent);
