import { useState, useRef, useCallback } from "react";
import { Upload, FileAudio, X } from "lucide-react";
import { cn } from "@/utils/cn";
import { Button } from "@/components/Button/Button";

const ACCEPTED_TYPES = [
  "audio/mpeg",
  "audio/mp3",
  "audio/wav",
  "audio/flac",
  "audio/ogg",
  "audio/mp4",
  "audio/x-m4a",
  "audio/webm",
  "video/mp4",
  "video/quicktime",
  "video/webm",
];

const ACCEPTED_EXTENSIONS = ".mp3,.wav,.flac,.ogg,.m4a,.webm,.mp4,.mov";

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

interface AudioDropZoneProps {
  file: File | null;
  onFileChange: (file: File | null) => void;
  disabled?: boolean;
  className?: string;
}

export function AudioDropZone({ file, onFileChange, disabled, className }: AudioDropZoneProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [dragOver, setDragOver] = useState(false);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setDragOver(false);
      const f = e.dataTransfer.files[0];
      if (
        f &&
        (ACCEPTED_TYPES.includes(f.type) || f.name.match(/\.(mp3|wav|flac|ogg|m4a|webm|mp4|mov)$/i))
      ) {
        onFileChange(f);
      }
    },
    [onFileChange]
  );

  if (file) {
    return (
      <div
        className={cn(
          "flex items-center gap-3 rounded-xl border border-border bg-card p-3",
          className
        )}
      >
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-primary/10">
          <FileAudio className="h-5 w-5 text-primary" />
        </div>
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium text-foreground">{file.name}</p>
          <p className="text-xs text-muted-foreground">{formatFileSize(file.size)}</p>
        </div>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 shrink-0 text-muted-foreground hover:text-destructive"
          onClick={() => onFileChange(null)}
          aria-label="Remove file"
        >
          <X className="h-4 w-4" />
        </Button>
      </div>
    );
  }

  return (
    <div className={className}>
      <button
        type="button"
        disabled={disabled}
        className={cn(
          "flex w-full flex-col items-center justify-center gap-3 rounded-xl border-2 border-dashed p-8",
          "text-muted-foreground",
          "motion-safe:transition-all motion-safe:duration-200",
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
          dragOver
            ? "border-primary bg-primary/5 scale-[1.01]"
            : "border-border hover:border-primary/50 hover:bg-muted/30",
          disabled && "cursor-not-allowed opacity-50"
        )}
        onClick={() => inputRef.current?.click()}
        onDragOver={(e) => {
          e.preventDefault();
          setDragOver(true);
        }}
        onDragLeave={() => setDragOver(false)}
        onDrop={handleDrop}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            inputRef.current?.click();
          }
        }}
        aria-label="Upload audio file"
      >
        <Upload className={cn("h-8 w-8", dragOver && "motion-safe:animate-bounce")} />
        <div className="text-center">
          <p className="text-sm font-medium">Drop audio or video file here</p>
          <p className="mt-0.5 text-xs">or click to browse</p>
        </div>
        <div className="flex flex-wrap justify-center gap-1">
          {["mp3", "wav", "flac", "ogg", "m4a", "webm", "mp4"].map((ext) => (
            <span
              key={ext}
              className="rounded bg-muted px-1.5 py-0.5 text-[10px] font-medium uppercase"
            >
              {ext}
            </span>
          ))}
        </div>
      </button>
      <input
        ref={inputRef}
        type="file"
        accept={ACCEPTED_EXTENSIONS}
        className="hidden"
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (f) onFileChange(f);
          e.target.value = "";
        }}
        aria-label="Upload audio file"
      />
    </div>
  );
}
