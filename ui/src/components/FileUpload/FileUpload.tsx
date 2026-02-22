import {
  File,
  FileText,
  FileCode,
  FileAudio,
  FileVideo,
  FileSpreadsheet,
  FileArchive,
  Paperclip,
  X,
} from "lucide-react";
import { useCallback, useMemo, useRef } from "react";

import { Button } from "@/components/Button/Button";
import { useConfig } from "@/config/ConfigProvider";
import { fileToBase64 } from "@/utils/fileToBase64";
import { formatBytes } from "@/utils/formatters";
import { cn } from "@/utils/cn";
import { isFileTypeAllowed, buildAcceptAttribute } from "@/utils/fileTypes";

import type { ChatFile } from "@/components/chat-types";

function getFileIcon(type: string, name: string) {
  // Check by MIME type first
  if (type.startsWith("image/")) return File; // Images show preview, this is fallback
  if (type.startsWith("video/")) return FileVideo;
  if (type.startsWith("audio/")) return FileAudio;
  if (type.startsWith("text/")) return FileText;

  // Check specific types
  if (type === "application/pdf") return FileText;
  if (type === "application/json") return FileCode;
  if (type.includes("spreadsheet") || type.includes("excel") || type === "text/csv")
    return FileSpreadsheet;
  if (
    type.includes("zip") ||
    type.includes("tar") ||
    type.includes("archive") ||
    type.includes("compressed")
  )
    return FileArchive;
  if (
    type.includes("javascript") ||
    type.includes("typescript") ||
    type.includes("html") ||
    type.includes("css")
  )
    return FileCode;

  // Check by extension
  const ext = name.split(".").pop()?.toLowerCase();
  if (ext) {
    const codeExts = [
      "js",
      "ts",
      "jsx",
      "tsx",
      "py",
      "rs",
      "go",
      "java",
      "c",
      "cpp",
      "h",
      "rb",
      "php",
      "swift",
      "kt",
    ];
    const textExts = ["txt", "md", "rst", "log"];
    const spreadsheetExts = ["csv", "xlsx", "xls", "ods"];
    const archiveExts = ["zip", "tar", "gz", "rar", "7z", "bz2"];

    if (codeExts.includes(ext)) return FileCode;
    if (textExts.includes(ext)) return FileText;
    if (spreadsheetExts.includes(ext)) return FileSpreadsheet;
    if (archiveExts.includes(ext)) return FileArchive;
  }

  return File;
}

interface FileUploadProps {
  files: ChatFile[];
  onFilesChange: (files: ChatFile[]) => void;
  disabled?: boolean;
}

export function FileUpload({ files, onFilesChange, disabled }: FileUploadProps) {
  const { config } = useConfig();
  const fileInputRef = useRef<HTMLInputElement>(null);

  const maxSize = config?.chat.max_file_size_bytes || 10 * 1024 * 1024;
  const allowedTypes = useMemo(
    () => config?.chat.allowed_file_types || [],
    [config?.chat.allowed_file_types]
  );

  const acceptAttribute = useMemo(() => buildAcceptAttribute(allowedTypes), [allowedTypes]);

  const handleFileSelect = useCallback(
    async (event: React.ChangeEvent<HTMLInputElement>) => {
      const selectedFiles = event.target.files;
      if (!selectedFiles) return;

      const newFiles: ChatFile[] = [];

      for (const file of Array.from(selectedFiles)) {
        // Check file size
        if (file.size > maxSize) {
          console.warn(`File ${file.name} exceeds max size`);
          continue;
        }

        // Check file type
        if (!isFileTypeAllowed(file, allowedTypes)) {
          console.warn(`File ${file.name} type not allowed`);
          continue;
        }

        const base64 = await fileToBase64(file);
        const chatFile: ChatFile = {
          id: crypto.randomUUID(),
          name: file.name,
          type: file.type,
          size: file.size,
          base64,
        };

        // Create preview for images
        if (file.type.startsWith("image/")) {
          chatFile.preview = `data:${file.type};base64,${base64}`;
        }

        newFiles.push(chatFile);
      }

      onFilesChange([...files, ...newFiles]);

      // Reset input
      if (fileInputRef.current) {
        fileInputRef.current.value = "";
      }
    },
    [files, onFilesChange, maxSize, allowedTypes]
  );

  const handleRemoveFile = useCallback(
    (fileId: string) => {
      onFilesChange(files.filter((f) => f.id !== fileId));
    },
    [files, onFilesChange]
  );

  const handleDrop = useCallback(
    (event: React.DragEvent) => {
      event.preventDefault();
      const droppedFiles = event.dataTransfer.files;
      if (droppedFiles.length > 0) {
        // Create a fake event to reuse the handler
        handleFileSelect({
          target: { files: droppedFiles },
        } as React.ChangeEvent<HTMLInputElement>);
      }
    },
    [handleFileSelect]
  );

  const handleDragOver = useCallback((event: React.DragEvent) => {
    event.preventDefault();
  }, []);

  if (!config?.chat.file_uploads_enabled) {
    return null;
  }

  return (
    <div className="space-y-2">
      {files.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {files.map((file) => (
            <div
              key={file.id}
              className="group relative flex items-center gap-2 rounded-lg border bg-muted/50 px-2 py-1"
            >
              {file.preview ? (
                <img
                  src={file.preview}
                  alt={file.name}
                  className="h-10 w-10 rounded object-cover"
                />
              ) : (
                (() => {
                  const Icon = getFileIcon(file.type, file.name);
                  return (
                    <div className="flex h-10 w-10 items-center justify-center rounded bg-muted">
                      <Icon className="h-5 w-5 text-muted-foreground" />
                    </div>
                  );
                })()
              )}
              <div className="flex flex-col">
                <span className="max-w-[120px] truncate text-xs font-medium">{file.name}</span>
                <span className="text-xs text-muted-foreground">{formatBytes(file.size)}</span>
              </div>
              <button
                onClick={() => handleRemoveFile(file.id)}
                aria-label={`Remove ${file.name}`}
                className="absolute -right-1 -top-1 rounded-full bg-destructive p-0.5 text-destructive-foreground opacity-0 transition-opacity group-hover:opacity-100"
              >
                <X className="h-3 w-3" />
              </button>
            </div>
          ))}
        </div>
      )}

      <div
        onDrop={handleDrop}
        onDragOver={handleDragOver}
        aria-disabled={disabled || undefined}
        className={cn("flex items-center gap-2", disabled && "pointer-events-none opacity-50")}
      >
        <input
          ref={fileInputRef}
          type="file"
          multiple
          accept={acceptAttribute}
          onChange={handleFileSelect}
          className="hidden"
          aria-label="Upload file"
        />
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={() => fileInputRef.current?.click()}
          disabled={disabled}
        >
          <Paperclip className="mr-2 h-4 w-4" />
          Attach
        </Button>
        <span className="text-xs text-muted-foreground">Max {formatBytes(maxSize)}</span>
      </div>
    </div>
  );
}
