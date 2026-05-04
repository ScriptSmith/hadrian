import { CheckCircle2, Cpu, Download, ExternalLink, Loader2, XCircle } from "lucide-react";
import { Button } from "@/components/Button/Button";
import { cn } from "@/utils/cn";
import type { LanguageModelAvailability } from "@/services/browser-ai";

export interface BrowserAiState {
  /** True if `globalThis.LanguageModel` is exposed by this browser. */
  supported: boolean;
  availability: LanguageModelAvailability;
  /** 0..1, only meaningful while a download is in progress. */
  downloadProgress: number | null;
  /** True while we are actively triggering or awaiting a download. */
  downloading: boolean;
  error: string | null;
}

interface BrowserAiCardProps {
  state: BrowserAiState;
  onDownload: () => void;
  className?: string;
}

export function BrowserAiCard({ state, onDownload, className }: BrowserAiCardProps) {
  if (!state.supported) {
    return (
      <div className={cn("rounded-lg border border-border bg-muted/30 p-4", className)}>
        <div className="flex items-center justify-between gap-3">
          <div className="min-w-0">
            <p className="text-sm font-medium">Browser AI</p>
            <p className="text-xs text-muted-foreground">
              On-device model running locally in your browser
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-1.5 text-xs text-muted-foreground">
            <XCircle className="h-3.5 w-3.5" />
            Not supported
          </div>
        </div>
        <p className="mt-2 text-xs text-muted-foreground">
          Open this page in Chrome 148+ (or recent Edge / Brave / other Chromium).{" "}
          <a
            href="https://developer.chrome.com/docs/ai/get-started"
            target="_blank"
            rel="noopener noreferrer"
            className="text-primary underline"
          >
            docs
            <ExternalLink className="ml-0.5 inline h-3 w-3" />
          </a>
        </p>
      </div>
    );
  }

  const isReady = state.availability === "available";
  const isDownloading = state.downloading || state.availability === "downloading";
  const isDownloadable = state.availability === "downloadable" && !isDownloading;
  const progressPercent =
    state.downloadProgress != null
      ? Math.max(0, Math.min(100, state.downloadProgress * 100))
      : null;

  return (
    <div
      className={cn(
        "rounded-lg border p-4",
        isReady
          ? "border-emerald-200 bg-emerald-50/60 dark:border-emerald-500/20 dark:bg-emerald-500/5"
          : "border-sky-200 bg-sky-50/60 dark:border-sky-500/20 dark:bg-sky-500/5",
        className
      )}
    >
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <p className="text-sm font-medium">Browser AI</p>
          <p className="text-xs text-muted-foreground">
            Runs locally on-device. Private, free, no API key.
          </p>
        </div>
        {isReady ? (
          <div className="flex shrink-0 items-center gap-1.5 text-sm text-emerald-700 dark:text-emerald-400">
            <CheckCircle2 className="h-4 w-4" />
            Ready
          </div>
        ) : isDownloading ? (
          <div className="flex shrink-0 items-center gap-1.5 text-xs text-sky-700 dark:text-sky-400">
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
            {progressPercent != null ? `${progressPercent.toFixed(0)}%` : "Downloading"}
          </div>
        ) : isDownloadable ? (
          <Button
            size="sm"
            onClick={onDownload}
            className="shrink-0 bg-sky-600 text-white hover:bg-sky-700 dark:bg-sky-600 dark:hover:bg-sky-500"
          >
            <Download className="mr-1.5 h-3.5 w-3.5" />
            Download
          </Button>
        ) : (
          <div className="flex shrink-0 items-center gap-1.5 text-xs text-muted-foreground">
            <Cpu className="h-3.5 w-3.5" />
            Unavailable
          </div>
        )}
      </div>

      {isDownloading && progressPercent != null && (
        <div
          className="mt-3 h-1 w-full overflow-hidden rounded-full bg-sky-100 dark:bg-sky-500/10"
          role="progressbar"
          aria-valuenow={progressPercent}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-label="Browser AI model download progress"
        >
          <div
            className="h-full rounded-full bg-sky-600 transition-[width] duration-200 dark:bg-sky-500"
            style={{ width: `${progressPercent}%` }}
          />
        </div>
      )}

      {state.error && (
        <p className="mt-2 flex items-start gap-1.5 text-xs text-destructive">
          <XCircle className="mt-0.5 h-3 w-3 shrink-0" />
          {state.error}
        </p>
      )}

      {state.availability === "unavailable" && !state.error && (
        <p className="mt-2 text-xs text-muted-foreground">
          The browser exposes the API but reports the device as ineligible (typically not enough
          memory, storage, or GPU). The model will appear here once your environment qualifies.
        </p>
      )}
    </div>
  );
}
