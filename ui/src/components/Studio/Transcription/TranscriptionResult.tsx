import { Copy, Download } from "lucide-react";
import { cn } from "@/utils/cn";
import { Button } from "@/components/Button/Button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";

interface TranscriptionResultProps {
  text: string;
  format: string;
  className?: string;
}

export function TranscriptionResult({ text, format, className }: TranscriptionResultProps) {
  const handleCopy = async () => {
    await navigator.clipboard.writeText(text);
  };

  const handleDownload = () => {
    const ext =
      format === "srt"
        ? ".srt"
        : format === "vtt"
          ? ".vtt"
          : format === "verbose_json" || format === "json"
            ? ".json"
            : ".txt";
    const blob = new Blob([text], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `transcription${ext}`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const isSrt = format === "srt";
  const isVtt = format === "vtt";
  const isJson = format === "json" || format === "verbose_json";

  return (
    <div className={cn("rounded-xl border border-border bg-card", className)}>
      {/* Header with actions */}
      <div className="flex items-center justify-between border-b px-4 py-2">
        <span className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
          Result
        </span>
        <div className="flex gap-1">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                onClick={handleCopy}
                aria-label="Copy result"
              >
                <Copy className="h-3.5 w-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Copy</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                onClick={handleDownload}
                aria-label="Download result"
              >
                <Download className="h-3.5 w-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Download</TooltipContent>
          </Tooltip>
        </div>
      </div>

      {/* Content */}
      <div className="max-h-[60vh] overflow-y-auto p-4">
        {isJson ? (
          <pre className="whitespace-pre-wrap text-sm font-mono text-foreground leading-relaxed">
            {(() => {
              try {
                return JSON.stringify(JSON.parse(text), null, 2);
              } catch {
                return text;
              }
            })()}
          </pre>
        ) : isSrt || isVtt ? (
          <pre className="whitespace-pre-wrap text-sm leading-relaxed">
            {text.split("\n").map((line, i) => {
              const isTimecode =
                /-->/.test(line) || /^\d+$/.test(line.trim()) || /^WEBVTT/.test(line);
              return (
                <span
                  key={i}
                  className={
                    isTimecode ? "font-mono text-muted-foreground text-xs" : "text-foreground"
                  }
                >
                  {line}
                  {"\n"}
                </span>
              );
            })}
          </pre>
        ) : (
          <p className="text-sm text-foreground leading-relaxed whitespace-pre-wrap">{text}</p>
        )}
      </div>
    </div>
  );
}
