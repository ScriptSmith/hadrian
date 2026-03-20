/**
 * DataFileUpload - Upload data files for SQL queries
 *
 * Allows users to upload CSV, Parquet, JSON, and DuckDB database files for querying
 * with DuckDB. Files are registered in-memory and reset on page reload.
 *
 * DuckDB database files are registered via BROWSER_FILEREADER protocol, which reads
 * lazily from the File handle on demand — no size limit, no memory overhead.
 */

import { useCallback, useRef, useState } from "react";
import { Upload, X, FileSpreadsheet, AlertCircle, Loader2, Eye } from "lucide-react";

import { duckdbService, type FileType } from "@/services/duckdb";
import {
  useChatUIStore,
  useDataFiles,
  type DataFile,
  type DataFileTable,
} from "@/stores/chatUIStore";
import { cn } from "@/utils/cn";
import {
  Modal,
  ModalHeader,
  ModalTitle,
  ModalDescription,
  ModalClose,
  ModalContent,
} from "@/components/Modal/Modal";

/** Accepted file extensions and their types */
const FILE_TYPE_MAP: Record<string, FileType> = {
  csv: "csv",
  parquet: "parquet",
  json: "json",
  duckdb: "duckdb",
};

/** File type to display name */
const FILE_TYPE_LABELS: Record<FileType, string> = {
  csv: "CSV",
  parquet: "Parquet",
  json: "JSON",
  duckdb: "DuckDB",
};

/** Max file size for flat files: 100MB (DuckDB databases use lazy reads, no limit) */
const MAX_FLAT_FILE_SIZE = 100 * 1024 * 1024;

/** Format file size for display */
function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
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
  /** Called when a file is successfully added (use for auto-enabling tool) */
  onFileAdded?: () => void;
}

export function DataFileUpload({
  className,
  disabled = false,
  compact = false,
  onFileAdded,
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

        // Validate file size (only for flat files — duckdb uses lazy file reads)
        if (fileType !== "duckdb" && file.size > MAX_FLAT_FILE_SIZE) {
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
          // For .duckdb files, pass the File handle directly — DuckDB reads lazily via
          // BROWSER_FILEREADER protocol (no memory overhead, no size limit).
          // For flat files, load into memory (capped at 100MB).
          const result =
            fileType === "duckdb"
              ? await duckdbService.registerDatabaseFile(file.name, file)
              : await duckdbService.registerFile(file.name, await file.arrayBuffer(), fileType);

          if (result.success) {
            if (fileType === "duckdb" && result.dbAlias) {
              // For DuckDB database files, enumerate tables and describe each
              const tablesResult = await duckdbService.execute(
                `SELECT table_schema, table_name FROM information_schema.tables WHERE table_catalog = '${result.dbAlias}' AND table_schema NOT IN ('information_schema', 'pg_catalog') ORDER BY table_schema, table_name`
              );
              if (tablesResult.success && tablesResult.rows.length > 0) {
                const tables = [];
                for (const row of tablesResult.rows) {
                  const tableName = String(row.table_name);
                  const schemaName = String(row.table_schema);
                  const safeTable = tableName.replace(/"/g, '""');
                  const safeSchema = schemaName.replace(/"/g, '""');
                  const colResult = await duckdbService.describeTable(
                    `"${result.dbAlias}"."${safeSchema}"."${safeTable}"`
                  );
                  tables.push({
                    tableName,
                    schemaName,
                    columns: colResult.success
                      ? colResult.columns.map((c) => ({ name: c.name, type: c.type }))
                      : [],
                  });
                }
                updateDataFileStatus(fileId, true, undefined, {
                  tables,
                  dbName: result.dbAlias,
                });
              } else {
                updateDataFileStatus(fileId, true, undefined, { dbName: result.dbAlias });
              }
            } else {
              // Flat file — get column schema
              const schemaResult = await duckdbService.describeTable(`'${file.name}'`);
              if (schemaResult.success && schemaResult.columns.length > 0) {
                updateDataFileStatus(fileId, true, undefined, {
                  columns: schemaResult.columns.map((c) => ({ name: c.name, type: c.type })),
                });
              } else {
                updateDataFileStatus(fileId, true);
              }
            }
            onFileAdded?.();
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
    [disabled, dataFiles, addDataFile, updateDataFileStatus, onFileAdded]
  );

  /** Handle file removal */
  const handleRemove = useCallback(
    async (file: DataFile) => {
      removeDataFile(file.id);

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
        e.target.value = "";
      }
    },
    [handleFiles]
  );

  const acceptedExtensions = Object.keys(FILE_TYPE_MAP)
    .map((ext) => `.${ext}`)
    .join(",");

  return (
    <div className={cn("space-y-2 min-w-0", className)}>
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
          <span className="text-[10px] text-muted-foreground">CSV, Parquet, JSON, DuckDB</span>
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
        <div className="flex flex-wrap gap-1.5 min-w-0">
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
  const [schemaOpen, setSchemaOpen] = useState(false);
  const hasError = !file.registered && file.error;
  const isLoading = !file.registered && !file.error;
  const hasColumns = file.columns && file.columns.length > 0;
  const hasTables = file.tables && file.tables.length > 0;
  const isDatabase = file.type === "duckdb";
  const hasSchema = hasColumns || hasTables;

  const chipDetail =
    isDatabase && hasTables
      ? `${file.tables!.length} table${file.tables!.length !== 1 ? "s" : ""}`
      : hasColumns
        ? `${file.columns!.length} cols`
        : undefined;

  return (
    <>
      <div
        className={cn(
          "flex items-center gap-1.5 px-2 py-1 rounded-md text-xs max-w-full min-w-0",
          hasError && "bg-destructive/10 text-destructive",
          !hasError && "bg-muted text-foreground"
        )}
      >
        <span className="shrink-0">
          {isLoading ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : hasError ? (
            <AlertCircle className="h-3 w-3" />
          ) : (
            <FileSpreadsheet className="h-3 w-3" />
          )}
        </span>
        <span className="truncate min-w-0">{file.name}</span>
        <span className="text-muted-foreground shrink-0">{FILE_TYPE_LABELS[file.type]}</span>
        {chipDetail && <span className="text-muted-foreground shrink-0">{chipDetail}</span>}
        {hasSchema && !isLoading && (
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              setSchemaOpen(true);
            }}
            className="shrink-0 p-0.5 rounded hover:bg-foreground/10 transition-colors"
            aria-label={`View schema for ${file.name}`}
          >
            <Eye className="h-3 w-3" />
          </button>
        )}
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onRemove();
          }}
          className="shrink-0 p-0.5 rounded hover:bg-foreground/10 transition-colors"
          aria-label={`Remove ${file.name}`}
        >
          <X className="h-3 w-3" />
        </button>
      </div>

      {/* Schema modal */}
      {hasSchema && (
        <DataSchemaModal open={schemaOpen} onClose={() => setSchemaOpen(false)} file={file} />
      )}
    </>
  );
}

/** Modal showing full schema for a data file */
function DataSchemaModal({
  open,
  onClose,
  file,
}: {
  open: boolean;
  onClose: () => void;
  file: DataFile;
}) {
  const isDatabase = file.type === "duckdb";
  const hasTables = file.tables && file.tables.length > 0;
  const hasColumns = file.columns && file.columns.length > 0;

  return (
    <Modal open={open} onClose={onClose} className="max-w-md">
      <ModalHeader>
        <ModalTitle>{file.name}</ModalTitle>
        <ModalDescription>
          {FILE_TYPE_LABELS[file.type]} &middot; {formatFileSize(file.size)}
          {isDatabase && file.dbName && (
            <>
              {" "}
              &middot; Attached as{" "}
              <code className="text-xs bg-muted px-1 rounded">{file.dbName}</code>
            </>
          )}
        </ModalDescription>
      </ModalHeader>
      <ModalClose onClose={onClose} />
      <ModalContent className="max-h-[60vh] overflow-y-auto">
        {/* Database tables */}
        {hasTables && (
          <div className="space-y-4">
            {file.tables!.map((table) => (
              <TableSchema key={table.tableName} table={table} dbName={file.dbName} />
            ))}
          </div>
        )}

        {/* Flat file columns */}
        {!isDatabase && hasColumns && (
          <div>
            <p className="text-xs text-muted-foreground mb-2 font-mono">
              SELECT * FROM &apos;{file.name}&apos;
            </p>
            <ColumnTable columns={file.columns!} />
          </div>
        )}
      </ModalContent>
    </Modal>
  );
}

/** Render a single table's schema */
function TableSchema({ table, dbName }: { table: DataFileTable; dbName?: string }) {
  return (
    <div>
      <div className="flex items-baseline gap-2 mb-1.5">
        <h3 className="text-sm font-medium font-mono">{table.tableName}</h3>
        <span className="text-xs text-muted-foreground">{table.columns.length} columns</span>
      </div>
      {dbName && (
        <p className="text-xs text-muted-foreground mb-2 font-mono">
          SELECT * FROM {dbName}.{table.schemaName}.{table.tableName}
        </p>
      )}
      {table.columns.length > 0 && <ColumnTable columns={table.columns} />}
    </div>
  );
}

/** Render a columns table */
function ColumnTable({ columns }: { columns: Array<{ name: string; type: string }> }) {
  return (
    <table className="w-full text-xs">
      <thead>
        <tr className="border-b border-border">
          <th className="text-left py-1 pr-4 font-medium text-muted-foreground">Column</th>
          <th className="text-left py-1 font-medium text-muted-foreground">Type</th>
        </tr>
      </thead>
      <tbody>
        {columns.map((col) => (
          <tr key={col.name} className="border-b border-border/50 last:border-0">
            <td className="py-1 pr-4 font-mono">{col.name}</td>
            <td className="py-1 text-muted-foreground">{col.type}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
