import { useCallback } from "react";
import { Download, Copy, Maximize2 } from "lucide-react";
import { cn } from "@/utils/cn";
import { Button } from "@/components/Button/Button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { ExpandableCaption } from "./ExpandableCaption";

interface ImageCardProps {
  imageData: string;
  prompt: string;
  revisedPrompt?: string;
  createdAt: number;
  /** Called when fullscreen is requested (index-based, managed by gallery) */
  onFullscreen?: () => void;
}

function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
}

export function ImageCard({
  imageData,
  prompt,
  revisedPrompt,
  createdAt,
  onFullscreen,
}: ImageCardProps) {
  const handleDownload = useCallback(() => {
    const a = document.createElement("a");
    a.href = imageData;
    a.download = `image-${Date.now()}.png`;
    a.click();
  }, [imageData]);

  const handleCopy = useCallback(async () => {
    try {
      const response = await fetch(imageData);
      const blob = await response.blob();
      await navigator.clipboard.write([new ClipboardItem({ [blob.type]: blob })]);
    } catch {
      await navigator.clipboard.writeText(imageData);
    }
  }, [imageData]);

  return (
    <div
      className={cn(
        "group relative overflow-hidden rounded-xl border border-border bg-card",
        "motion-safe:transition-all motion-safe:duration-200",
        "hover:shadow-lg hover:border-border/80",
        "motion-safe:hover:scale-[1.02]"
      )}
    >
      <div className="relative aspect-square">
        <img
          src={imageData}
          alt={revisedPrompt ?? prompt}
          className="h-full w-full object-cover"
          loading="lazy"
        />

        {/* Timestamp badge */}
        <div className="absolute right-2 top-2 z-10 rounded-md bg-black/50 px-1.5 py-0.5 text-[10px] text-white backdrop-blur-sm">
          {formatTime(createdAt)}
        </div>

        {/* Hover/focus overlay with actions */}
        <div
          className={cn(
            "absolute inset-0 z-20 flex items-center justify-center gap-2 bg-black/40 backdrop-blur-[2px]",
            "opacity-0 group-hover:opacity-100 focus-within:opacity-100",
            "motion-safe:transition-opacity motion-safe:duration-200"
          )}
        >
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="h-9 w-9 bg-white/20 text-white hover:bg-white/30"
                onClick={handleDownload}
                aria-label="Download image"
              >
                <Download className="h-4 w-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Download</TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="h-9 w-9 bg-white/20 text-white hover:bg-white/30"
                onClick={handleCopy}
                aria-label="Copy image"
              >
                <Copy className="h-4 w-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Copy</TooltipContent>
          </Tooltip>

          {onFullscreen && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-9 w-9 bg-white/20 text-white hover:bg-white/30"
                  onClick={onFullscreen}
                  aria-label="View fullscreen"
                >
                  <Maximize2 className="h-4 w-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Fullscreen</TooltipContent>
            </Tooltip>
          )}
        </div>
      </div>

      {/* Caption */}
      {revisedPrompt && (
        <div className="border-t px-3 py-2">
          <ExpandableCaption text={revisedPrompt} />
        </div>
      )}
    </div>
  );
}
