/**
 * ImageArtifact - Image Display with Download
 *
 * Renders images from tools like matplotlib, image generation, etc.
 * Supports base64 data URLs and regular URLs with download functionality.
 */

import { memo, useState, useEffect, useCallback } from "react";
import { createPortal } from "react-dom";
import { Download, ZoomIn, ZoomOut, Maximize2, X } from "lucide-react";

import type { Artifact } from "@/components/chat-types";
import { Button } from "@/components/Button/Button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { cn } from "@/utils/cn";

export interface ImageArtifactProps {
  artifact: Artifact;
  className?: string;
}

/** Extract image source from artifact data */
function getImageSource(data: unknown): string | null {
  if (typeof data === "string") {
    return data;
  }
  if (typeof data === "object" && data !== null) {
    const obj = data as Record<string, unknown>;
    if (typeof obj.src === "string") return obj.src;
    if (typeof obj.url === "string") return obj.url;
    if (typeof obj.data === "string") return obj.data;
  }
  return null;
}

/** Get filename from artifact or generate one */
function getFilename(artifact: Artifact): string {
  if (artifact.title) {
    // Sanitize title for filename
    const sanitized = artifact.title.replace(/[^a-zA-Z0-9-_]/g, "_");
    const ext = artifact.mimeType?.split("/")[1] || "png";
    return `${sanitized}.${ext}`;
  }
  const ext = artifact.mimeType?.split("/")[1] || "png";
  return `image-${artifact.id}.${ext}`;
}

function ImageArtifactComponent({ artifact, className }: ImageArtifactProps) {
  const [isZoomed, setIsZoomed] = useState(false);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [loadError, setLoadError] = useState(false);

  const src = getImageSource(artifact.data);

  if (!src) {
    return <div className="p-4 text-sm text-muted-foreground">Invalid image artifact data</div>;
  }

  const handleDownload = () => {
    const link = document.createElement("a");
    link.href = src;
    link.download = getFilename(artifact);
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  };

  const toggleZoom = () => setIsZoomed(!isZoomed);
  const toggleFullscreen = () => setIsFullscreen(!isFullscreen);

  if (loadError) {
    return <div className="p-4 text-sm text-muted-foreground">Failed to load image</div>;
  }

  return (
    <>
      <div className={cn("relative group", className)}>
        {/* Action buttons */}
        <div className="absolute right-2 top-2 flex items-center gap-1 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity z-10">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="secondary"
                size="sm"
                className="h-7 w-7 p-0"
                onClick={toggleZoom}
                aria-label={isZoomed ? "Zoom out" : "Zoom in"}
              >
                {isZoomed ? (
                  <ZoomOut className="h-3.5 w-3.5" />
                ) : (
                  <ZoomIn className="h-3.5 w-3.5" />
                )}
              </Button>
            </TooltipTrigger>
            <TooltipContent>{isZoomed ? "Zoom out" : "Zoom in"}</TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="secondary"
                size="sm"
                className="h-7 w-7 p-0"
                onClick={toggleFullscreen}
                aria-label="View fullscreen"
              >
                <Maximize2 className="h-3.5 w-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>View fullscreen</TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="secondary"
                size="sm"
                className="h-7 w-7 p-0"
                onClick={handleDownload}
                aria-label="Download image"
              >
                <Download className="h-3.5 w-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Download</TooltipContent>
          </Tooltip>
        </div>

        {/* Image container */}
        <div
          className={cn(
            "flex items-center justify-center p-4",
            "overflow-auto",
            isZoomed ? "max-h-none" : "max-h-[400px]"
          )}
        >
          <button
            type="button"
            onClick={toggleZoom}
            aria-label={isZoomed ? "Zoom out" : "Zoom in"}
            className={cn(
              "appearance-none bg-transparent border-0 p-0 rounded",
              isZoomed ? "cursor-zoom-out" : "cursor-zoom-in"
            )}
          >
            <img
              src={src}
              alt={artifact.title || "Generated image"}
              className={cn(
                "rounded transition-transform duration-200",
                isZoomed ? "max-w-none" : "max-w-full max-h-[360px]"
              )}
              onError={() => setLoadError(true)}
            />
          </button>
        </div>
      </div>

      {/* Fullscreen modal - rendered via portal */}
      {isFullscreen && (
        <FullscreenModal
          src={src}
          artifact={artifact}
          onClose={toggleFullscreen}
          onDownload={handleDownload}
        />
      )}
    </>
  );
}

/** Fullscreen image modal - uses portal and locks body scroll */
function FullscreenModal({
  src,
  artifact,
  onClose,
  onDownload,
}: {
  src: string;
  artifact: Artifact;
  onClose: () => void;
  onDownload: () => void;
}) {
  // Handle escape key
  const handleEscape = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    },
    [onClose]
  );

  // Lock body scroll and listen for escape
  useEffect(() => {
    document.body.style.overflow = "hidden";
    document.addEventListener("keydown", handleEscape);
    return () => {
      document.body.style.overflow = "";
      document.removeEventListener("keydown", handleEscape);
    };
  }, [handleEscape]);

  return createPortal(
    // eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- fullscreen overlay; Escape key handler registered separately
    <div
      className="fixed inset-0 z-[200] flex items-center justify-center bg-black/90"
      onClick={onClose}
    >
      <Button variant="secondary" size="sm" className="absolute right-4 top-4" onClick={onClose}>
        <X className="h-4 w-4 mr-1" />
        Close
      </Button>

      <Button
        variant="secondary"
        size="sm"
        className="absolute right-4 bottom-4"
        onClick={(e) => {
          e.stopPropagation();
          onDownload();
        }}
      >
        <Download className="h-4 w-4 mr-1" />
        Download
      </Button>

      {/* eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-noninteractive-element-interactions -- stopPropagation prevents closing overlay when clicking image */}
      <img
        src={src}
        alt={artifact.title || "Generated image"}
        className="max-w-[90vw] max-h-[90vh] object-contain"
        onClick={(e) => e.stopPropagation()}
      />
    </div>,
    document.body
  );
}

export const ImageArtifact = memo(ImageArtifactComponent);
