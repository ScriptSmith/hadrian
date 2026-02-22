import { useEffect, useCallback } from "react";
import { X, ChevronLeft, ChevronRight, Download, Copy } from "lucide-react";
import { Button } from "@/components/Button/Button";
import { ExpandableCaption } from "./ExpandableCaption";

export interface LightboxImage {
  imageData: string;
  prompt: string;
  revisedPrompt?: string;
  modelLabel?: string;
}

interface ImageLightboxProps {
  images: LightboxImage[];
  currentIndex: number;
  onClose: () => void;
  onNavigate: (index: number) => void;
}

export function ImageLightbox({ images, currentIndex, onClose, onNavigate }: ImageLightboxProps) {
  const image = images[currentIndex];
  const hasPrev = currentIndex > 0;
  const hasNext = currentIndex < images.length - 1;

  const handlePrev = useCallback(() => {
    if (hasPrev) onNavigate(currentIndex - 1);
  }, [hasPrev, currentIndex, onNavigate]);

  const handleNext = useCallback(() => {
    if (hasNext) onNavigate(currentIndex + 1);
  }, [hasNext, currentIndex, onNavigate]);

  const handleDownload = useCallback(() => {
    const a = document.createElement("a");
    a.href = image.imageData;
    a.download = `image-${Date.now()}.png`;
    a.click();
  }, [image.imageData]);

  const handleCopy = useCallback(async () => {
    try {
      const response = await fetch(image.imageData);
      const blob = await response.blob();
      await navigator.clipboard.write([new ClipboardItem({ [blob.type]: blob })]);
    } catch {
      await navigator.clipboard.writeText(image.imageData);
    }
  }, [image.imageData]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      else if (e.key === "ArrowLeft") handlePrev();
      else if (e.key === "ArrowRight") handleNext();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose, handlePrev, handleNext]);

  if (!image) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex flex-col bg-black/90 backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      aria-label="Image lightbox"
    >
      {/* Top toolbar */}
      <div className="flex items-center justify-between px-4 py-3">
        <div className="flex items-center gap-3">
          {image.modelLabel && (
            <span className="rounded-md bg-white/10 px-2.5 py-1 text-xs font-medium text-white">
              {image.modelLabel}
            </span>
          )}
          <span className="text-sm text-white/70">
            {currentIndex + 1} of {images.length}
          </span>
        </div>

        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="icon"
            className="h-9 w-9 text-white hover:bg-white/15"
            onClick={handleDownload}
            aria-label="Download image"
          >
            <Download className="h-4 w-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-9 w-9 text-white hover:bg-white/15"
            onClick={handleCopy}
            aria-label="Copy image"
          >
            <Copy className="h-4 w-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-9 w-9 text-white hover:bg-white/15"
            onClick={onClose}
            aria-label="Close lightbox"
          >
            <X className="h-5 w-5" />
          </Button>
        </div>
      </div>

      {/* Image + navigation area */}
      <div className="relative flex flex-1 items-center justify-center px-16">
        {/* Backdrop close */}
        <button
          type="button"
          onClick={onClose}
          className="absolute inset-0 z-0"
          aria-label="Close lightbox"
        >
          <span className="sr-only">Close</span>
        </button>

        {/* Left arrow */}
        {hasPrev && (
          <Button
            variant="ghost"
            size="icon"
            className="absolute left-4 z-10 h-10 w-10 rounded-full bg-white/10 text-white hover:bg-white/20"
            onClick={handlePrev}
            aria-label="Previous image"
          >
            <ChevronLeft className="h-5 w-5" />
          </Button>
        )}

        {/* Image */}
        <img
          src={image.imageData}
          alt={image.revisedPrompt ?? image.prompt}
          className="relative z-10 max-h-[75vh] max-w-[85vw] rounded-lg object-contain"
        />

        {/* Right arrow */}
        {hasNext && (
          <Button
            variant="ghost"
            size="icon"
            className="absolute right-4 z-10 h-10 w-10 rounded-full bg-white/10 text-white hover:bg-white/20"
            onClick={handleNext}
            aria-label="Next image"
          >
            <ChevronRight className="h-5 w-5" />
          </Button>
        )}
      </div>

      {/* Bottom caption */}
      {image.revisedPrompt && (
        <div className="px-8 pb-4 pt-2">
          <ExpandableCaption
            text={image.revisedPrompt}
            className="text-white/80 [&_button]:text-white/60 [&_button]:hover:text-white"
          />
        </div>
      )}
    </div>
  );
}
