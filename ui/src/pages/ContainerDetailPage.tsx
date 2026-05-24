import { useCallback, useMemo, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { createColumnHelper, type ColumnDef } from "@tanstack/react-table";
import { ArrowLeft, Box, Download, FileText, Trash2 } from "lucide-react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";

import {
  apiV1ContainersGetOptions,
  apiV1ContainersListFilesOptions,
  apiV1ContainersFileDeleteMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import { Badge } from "@/components/Badge/Badge";
import { Button } from "@/components/Button/Button";
import { Card, CardContent } from "@/components/Card/Card";
import { DataTable } from "@/components/DataTable/DataTable";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { useToast } from "@/components/Toast/Toast";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";
import { useAuth } from "@/auth";
import { formatDateTime, formatBytes } from "@/utils/formatters";
import { formatApiError } from "@/utils/formatApiError";
import type {
  Container,
  ContainerFile,
  ContainerFileList,
  ContainerStatus,
} from "@/pages/containers/types";

const STATUS_VARIANT: Record<ContainerStatus, "default" | "outline" | "destructive"> = {
  active: "default",
  expired: "outline",
  deleted: "destructive",
};

function Stat({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div>
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className="text-sm font-medium mt-0.5">{value}</p>
    </div>
  );
}

const columnHelper = createColumnHelper<ContainerFile>();

export default function ContainerDetailPage() {
  const { containerId = "" } = useParams();
  const navigate = useNavigate();
  const { token } = useAuth();
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const [downloadingId, setDownloadingId] = useState<string | null>(null);

  const {
    data: containerData,
    isLoading: containerLoading,
    error: containerError,
  } = useQuery({
    ...apiV1ContainersGetOptions({ path: { container_id: containerId } }),
    enabled: !!containerId,
  });
  const container = containerData as Container | undefined;

  const { data: filesData, isLoading: filesLoading } = useQuery({
    ...apiV1ContainersListFilesOptions({
      path: { container_id: containerId },
      query: { limit: 1000 },
    }),
    enabled: !!containerId,
  });
  const files = useMemo(
    () => (filesData as ContainerFileList | undefined)?.data ?? [],
    [filesData]
  );

  const deleteFileMutation = useMutation({
    ...apiV1ContainersFileDeleteMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "apiV1ContainersListFiles" }] });
      toast({ title: "File deleted", type: "success" });
    },
    onError: (err) => {
      toast({ title: "Failed to delete file", description: formatApiError(err), type: "error" });
    },
  });

  const downloadFile = useCallback(
    async (file: ContainerFile) => {
      setDownloadingId(file.id);
      try {
        const res = await fetch(`/api/v1/containers/${containerId}/files/${file.id}/content`, {
          headers: token ? { Authorization: `Bearer ${token}` } : {},
        });
        if (!res.ok) throw new Error(`Download failed (${res.status})`);
        const blob = await res.blob();
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = file.filename;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
      } catch (err) {
        toast({
          title: "Failed to download file",
          description: formatApiError(err),
          type: "error",
        });
      } finally {
        setDownloadingId(null);
      }
    },
    [containerId, token, toast]
  );

  const handleDeleteFile = async (file: ContainerFile) => {
    const ok = await confirm({
      title: "Delete file?",
      message: `Delete ${file.filename} from this container? This cannot be undone.`,
      confirmLabel: "Delete",
      variant: "destructive",
    });
    if (ok) {
      deleteFileMutation.mutate({ path: { container_id: containerId, file_id: file.id } });
    }
  };

  const columns = useMemo(
    () => [
      columnHelper.accessor("filename", {
        header: "File",
        cell: (ctx) => (
          <div className="flex items-center gap-2 min-w-0">
            <FileText className="h-4 w-4 text-muted-foreground shrink-0" />
            <span className="truncate font-mono text-xs" title={ctx.row.original.path}>
              {ctx.getValue()}
            </span>
          </div>
        ),
      }),
      columnHelper.accessor("source", {
        header: "Source",
        cell: (ctx) => (
          <Badge variant={ctx.getValue() === "assistant" ? "default" : "secondary"}>
            {ctx.getValue()}
          </Badge>
        ),
      }),
      columnHelper.accessor("bytes", {
        header: "Size",
        cell: (ctx) => <span className="text-sm">{formatBytes(ctx.getValue())}</span>,
      }),
      columnHelper.accessor("created_at", {
        header: "Created",
        cell: (ctx) => (
          <span className="text-sm text-muted-foreground">
            {formatDateTime(new Date(ctx.getValue() * 1000))}
          </span>
        ),
      }),
      columnHelper.display({
        id: "actions",
        header: "",
        cell: (ctx) => {
          const file = ctx.row.original;
          return (
            <div className="flex items-center justify-end gap-1">
              <Button
                size="icon"
                variant="ghost"
                className="h-7 w-7"
                onClick={() => downloadFile(file)}
                disabled={downloadingId === file.id}
                aria-label={`Download ${file.filename}`}
              >
                <Download className="h-4 w-4" />
              </Button>
              <Button
                size="icon"
                variant="ghost"
                className="h-7 w-7 text-muted-foreground hover:text-destructive"
                onClick={() => handleDeleteFile(file)}
                disabled={deleteFileMutation.isPending}
                aria-label={`Delete ${file.filename}`}
              >
                <Trash2 className="h-4 w-4" />
              </Button>
            </div>
          );
        },
      }),
    ],
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [downloadingId, deleteFileMutation.isPending]
  );

  return (
    <div className="p-6 max-w-5xl mx-auto">
      <Button variant="ghost" className="mb-4 -ml-2" onClick={() => navigate("/containers")}>
        <ArrowLeft className="h-4 w-4 mr-2" />
        Containers
      </Button>

      {containerError && (
        <div className="rounded-md bg-destructive/10 px-4 py-3 text-sm text-destructive">
          Failed to load container. It may have been deleted.
        </div>
      )}

      {containerLoading && <Skeleton className="h-32 w-full" />}

      {container && (
        <>
          <div className="flex items-start justify-between gap-3 mb-4">
            <div className="flex items-center gap-2 min-w-0">
              <Box className="h-6 w-6 text-muted-foreground shrink-0" />
              <div className="min-w-0">
                <h1 className="text-xl font-semibold truncate">
                  {container.name?.trim() || container.id}
                </h1>
                <p className="text-xs text-muted-foreground font-mono truncate">{container.id}</p>
              </div>
            </div>
            <Badge variant={STATUS_VARIANT[container.status] ?? "outline"}>
              {container.status}
            </Badge>
          </div>

          <Card className="mb-6">
            <CardContent className="p-4 grid grid-cols-2 gap-4 sm:grid-cols-3">
              <Stat label="Runtime" value={container.runtime} />
              <Stat label="Memory" value={container.memory_limit ?? "default"} />
              <Stat label="Idle TTL" value={`${Math.round(container.idle_ttl_secs / 60)} min`} />
              <Stat label="Created" value={formatDateTime(new Date(container.created_at * 1000))} />
              <Stat
                label="Last active"
                value={formatDateTime(new Date(container.last_active_at * 1000))}
              />
              <Stat
                label={container.status === "active" ? "Expires" : "Expired"}
                value={formatDateTime(new Date(container.expires_at * 1000))}
              />
            </CardContent>
          </Card>

          <h2 className="text-lg font-medium mb-3">Files</h2>
          <DataTable
            columns={columns as ColumnDef<ContainerFile>[]}
            data={files}
            isLoading={filesLoading}
            emptyMessage="No files in this container."
          />
        </>
      )}
    </div>
  );
}
