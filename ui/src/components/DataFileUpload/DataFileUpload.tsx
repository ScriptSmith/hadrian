/**
 * DataFileUpload - Upload data files for SQL queries
 *
 * Allows users to upload CSV, Parquet, and JSON files for querying
 * with DuckDB. Files are registered in-memory and reset on page reload.
 *
 * Note: SQLite files are NOT supported in DuckDB-WASM due to extension limitations.
 */

import { useCallback, useRef, useState } from "react";
import { Upload, X, FileSpreadsheet, AlertCircle, Loader2 } from "lucide-react";

import { duckdbService, type FileType } from "@/services/duckdb";
import { useChatUIStore, useDataFiles, type DataFile } from "@/stores/chatUIStore";
import { cn } from "@/utils/cn";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";

/** Accepted file extensions and their types */
const FILE_TYPE_MAP: Record<string, FileType> = {
  csv: "csv",
  parquet: "parquet",
  json: "json",
};

/** File type to display name */
const FILE_TYPE_LABELS: Record<FileType, string> = {
  csv: "CSV",
  parquet: "Parquet",
  json: "JSON",
};

/** Max file size: 100MB */
const MAX_FILE_SIZE = 100 * 1024 * 1024;

/** Format file size for display */
function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Get file extension from filename */
function getFileExtension(filename: string): string {
  const parts = filename.split(".");
  return parts.length > 1 ? parts.pop()!.toLowerCase() : "";
}

/** Generate unique ID for file */
function generateFileId(): string {
  return `file-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
}

export interface DataFileUploadProps {
  /** Optional className for the container */
  className?: string;
  /** Whether the upload is disabled */
  disabled?: boolean;
  /** Compact mode - show only chips, no drop zone */
  compact?: boolean;
}

export function DataFileUpload({
  className,
  disabled = false,
  compact = false,
}: DataFileUploadProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [isUploading, setIsUploading] = useState(false);

  const dataFiles = useDataFiles();
  const { addDataFile, removeDataFile, updateDataFileStatus } = useChatUIStore();

  /** Handle file selection */
  const handleFiles = useCallback(
    async (files: FileList | File[]) => {
      if (disabled) return;

      const fileArray = Array.from(files);
      setIsUploading(true);

      for (const file of fileArray) {
        const ext = getFileExtension(file.name);
        const fileType = FILE_TYPE_MAP[ext];

        // Validate file type
        if (!fileType) {
          console.warn(`Unsupported file type: ${ext}`);
          continue;
        }

        // Validate file size
        if (file.size > MAX_FILE_SIZE) {
          console.warn(`File too large: ${file.name} (${formatFileSize(file.size)})`);
          continue;
        }

        // Check for duplicate filename
        if (dataFiles.some((f: DataFile) => f.name === file.name)) {
          console.warn(`File already uploaded: ${file.name}`);
          continue;
        }

        // Create file entry
        const fileId = generateFileId();
        const dataFile: DataFile = {
          id: fileId,
          name: file.name,
          type: fileType,
          size: file.size,
          uploadedAt: Date.now(),
          registered: false,
        };

        // Add to store immediately (shows loading state)
        addDataFile(dataFile);

        // Register with DuckDB
        try {
          const buffer = await file.arrayBuffer();
          const result = await duckdbService.registerFile(file.name, buffer, fileType);

          if (result.success) {
            // Get column schema for the file
            // Use quoted filename for DuckDB (files are accessed as 'filename.ext')
            const schemaResult = await duckdbService.describeTable(`'${file.name}'`);
            if (schemaResult.success && schemaResult.columns.length > 0) {
              updateDataFileStatus(fileId, true, undefined, {
                columns: schemaResult.columns.map((c) => ({ name: c.name, type: c.type })),
              });
            } else {
              // Registration succeeded but schema extraction failed - still mark as registered
              updateDataFileStatus(fileId, true);
            }
          } else {
            updateDataFileStatus(fileId, false, result.error || "Registration failed");
          }
        } catch (error) {
          const errorMsg = error instanceof Error ? error.message : String(error);
          updateDataFileStatus(fileId, false, errorMsg);
        }
      }

      setIsUploading(false);
    },
    [disabled, dataFiles, addDataFile, updateDataFileStatus]
  );

  /** Handle file removal */
  const handleRemove = useCallback(
    async (file: DataFile) => {
      // Remove from store
      removeDataFile(file.id);

      // Unregister from DuckDB if it was registered
      if (file.registered) {
        try {
          await duckdbService.unregisterFile(file.name);
        } catch (error) {
          console.warn(`Failed to unregister file: ${file.name}`, error);
        }
      }
    },
    [removeDataFile]
  );

  /** Handle drag events */
  const handleDragOver = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      if (!disabled) setIsDragging(true);
    },
    [disabled]
  );

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setIsDragging(false);
      if (!disabled && e.dataTransfer.files.length > 0) {
        handleFiles(e.dataTransfer.files);
      }
    },
    [disabled, handleFiles]
  );

  /** Handle click to open file picker */
  const handleClick = useCallback(() => {
    if (!disabled) {
      inputRef.current?.click();
    }
  }, [disabled]);

  /** Handle file input change */
  const handleInputChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      if (e.target.files && e.target.files.length > 0) {
        handleFiles(e.target.files);
        // Reset input so same file can be selected again
        e.target.value = "";
      }
    },
    [handleFiles]
  );

  const acceptedExtensions = Object.keys(FILE_TYPE_MAP)
    .map((ext) => `.${ext}`)
    .join(",");

  return (
    <div className={cn("space-y-2", className)}>
      {/* Hidden file input */}
      <input
        ref={inputRef}
        type="file"
        accept={acceptedExtensions}
        multiple
        onChange={handleInputChange}
        className="hidden"
        disabled={disabled}
        aria-label="Upload data file"
      />

      {/* Drop zone (not shown in compact mode) */}
      {!compact && (
        <div
          role="button"
          tabIndex={disabled ? -1 : 0}
          aria-disabled={disabled || undefined}
          onClick={handleClick}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              handleClick();
            }
          }}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
          className={cn(
            "relative flex flex-col items-center justify-center gap-1 rounded-lg border-2 border-dashed p-4 transition-colors cursor-pointer",
            isDragging && "border-primary bg-primary/5",
            !isDragging && "border-muted-foreground/25 hover:border-muted-foreground/50",
            disabled && "opacity-50 cursor-not-allowed"
          )}
        >
          {isUploading ? (
            <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
          ) : (
            <Upload className="h-5 w-5 text-muted-foreground" />
          )}
          <span className="text-xs text-muted-foreground text-center">
            {isDragging ? "Drop files here" : "Drop files or click to upload"}
          </span>
          <span className="text-[10px] text-muted-foreground">CSV, Parquet, JSON (max 100MB)</span>
        </div>
      )}

      {/* Compact mode: just show upload button */}
      {compact && dataFiles.length === 0 && (
        <button
          type="button"
          onClick={handleClick}
          disabled={disabled || isUploading}
          className={cn(
            "flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors",
            disabled && "opacity-50 cursor-not-allowed"
          )}
        >
          {isUploading ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : (
            <Upload className="h-3.5 w-3.5" />
          )}
          <span>Upload data file</span>
        </button>
      )}

      {/* File chips */}
      {dataFiles.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {dataFiles.map((file: DataFile) => (
            <FileChip key={file.id} file={file} onRemove={() => handleRemove(file)} />
          ))}
          {/* Add more button in compact mode */}
          {compact && (
            <button
              type="button"
              onClick={handleClick}
              disabled={disabled || isUploading}
              aria-label="Add files"
              className={cn(
                "flex items-center gap-1 px-2 py-1 rounded-md text-xs",
                "bg-muted/50 hover:bg-muted text-muted-foreground hover:text-foreground",
                "transition-colors",
                disabled && "opacity-50 cursor-not-allowed"
              )}
            >
              {isUploading ? (
                <Loader2 className="h-3 w-3 animate-spin" />
              ) : (
                <Upload className="h-3 w-3" />
              )}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

/** Individual file chip */
function FileChip({ file, onRemove }: { file: DataFile; onRemove: () => void }) {
  const hasError = !file.registered && file.error;
  const isLoading = !file.registered && !file.error;
  const hasColumns = file.columns && file.columns.length > 0;

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <div
          className={cn(
            "flex items-center gap-1.5 px-2 py-1 rounded-md text-xs",
            hasError && "bg-destructive/10 text-destructive",
            !hasError && "bg-muted text-foreground"
          )}
        >
          {isLoading ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : hasError ? (
            <AlertCircle className="h-3 w-3" />
          ) : (
            <FileSpreadsheet className="h-3 w-3" />
          )}
          <span className="max-w-[120px] truncate">{file.name}</span>
          <span className="text-muted-foreground">{FILE_TYPE_LABELS[file.type]}</span>
          {hasColumns && <span className="text-muted-foreground">{file.columns!.length} cols</span>}
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              onRemove();
            }}
            className="ml-0.5 p-0.5 rounded hover:bg-foreground/10 transition-colors"
            aria-label={`Remove ${file.name}`}
          >
            <X className="h-3 w-3" />
          </button>
        </div>
      </TooltipTrigger>
      <TooltipContent side="bottom" className="max-w-sm">
        <div className="space-y-1.5">
          <p className="font-medium">{file.name}</p>
          <p className="text-xs text-muted-foreground">
            {FILE_TYPE_LABELS[file.type]} &middot; {formatFileSize(file.size)}
          </p>
          {hasError && <p className="text-xs text-destructive">{file.error}</p>}
          {!hasError && !isLoading && (
            <>
              <p className="text-xs text-muted-foreground">
                Query with: SELECT * FROM &apos;{file.name}&apos;
              </p>

              {/* File columns */}
              {hasColumns && (
                <div className="pt-1 border-t border-border/50">
                  <p className="text-xs font-medium mb-1">Columns:</p>
                  <div className="text-xs text-muted-foreground space-y-0.5 max-h-32 overflow-y-auto">
                    {file.columns!.map((col) => (
                      <div key={col.name} className="flex justify-between gap-2">
                        <span className="font-mono truncate">{col.name}</span>
                        <span className="text-muted-foreground shrink-0">{col.type}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </>
          )}
        </div>
      </TooltipContent>
    </Tooltip>
  );
}
