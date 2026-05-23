import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { Box, Calendar, Clock, Cpu, Trash2 } from "lucide-react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";

import {
  apiV1ContainersListOptions,
  apiV1ContainersDeleteMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import { Button } from "@/components/Button/Button";
import { Badge } from "@/components/Badge/Badge";
import { Card, CardContent } from "@/components/Card/Card";
import { Input } from "@/components/Input/Input";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";
import { useToast } from "@/components/Toast/Toast";
import { formatDateTime, formatRelativeTime } from "@/utils/formatters";
import { formatApiError } from "@/utils/formatApiError";
import type { Container, ContainerList, ContainerStatus } from "@/pages/containers/types";

function StatusBadge({ status }: { status: ContainerStatus }) {
  const variants: Record<ContainerStatus, "default" | "secondary" | "destructive" | "outline"> = {
    active: "default",
    expired: "outline",
    deleted: "destructive",
  };
  const labels: Record<ContainerStatus, string> = {
    active: "Active",
    expired: "Expired",
    deleted: "Deleted",
  };
  return <Badge variant={variants[status] ?? "outline"}>{labels[status] ?? status}</Badge>;
}

function ContainerCard({
  container,
  onDelete,
  deleting,
}: {
  container: Container;
  onDelete: (c: Container) => void;
  deleting: boolean;
}) {
  const title = container.name?.trim() || container.id;
  const isDeleted = container.status === "deleted";
  return (
    <Card className="h-full">
      <CardContent className="p-4">
        <div className="flex items-start justify-between gap-2">
          <Link
            to={`/containers/${container.id}`}
            className="flex items-center gap-2 min-w-0 hover:underline"
          >
            <Box className="h-5 w-5 text-muted-foreground shrink-0" />
            <span className="font-medium truncate" title={title}>
              {title}
            </span>
          </Link>
          <div className="flex items-center gap-1 shrink-0">
            <StatusBadge status={container.status} />
            {!isDeleted && (
              <Button
                size="icon"
                variant="ghost"
                className="h-7 w-7 text-muted-foreground hover:text-destructive"
                onClick={() => onDelete(container)}
                disabled={deleting}
                aria-label={`Delete container ${title}`}
              >
                <Trash2 className="h-4 w-4" />
              </Button>
            )}
          </div>
        </div>

        {container.name && (
          <p className="mt-1 text-xs text-muted-foreground font-mono truncate">{container.id}</p>
        )}

        <div className="mt-3 flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
          <span className="flex items-center gap-1">
            <Cpu className="h-3 w-3" />
            {container.runtime}
            {container.memory_limit && ` · ${container.memory_limit}`}
          </span>
          <span className="flex items-center gap-1">
            <Calendar className="h-3 w-3" />
            {formatDateTime(new Date(container.created_at * 1000))}
          </span>
          {container.status === "active" && (
            <span
              className="flex items-center gap-1"
              title={formatDateTime(new Date(container.expires_at * 1000))}
            >
              <Clock className="h-3 w-3" />
              Expires {formatRelativeTime(new Date(container.expires_at * 1000))}
            </span>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

function ContainerCardSkeleton() {
  return (
    <Card>
      <CardContent className="p-4">
        <div className="flex items-start justify-between gap-2">
          <div className="flex items-center gap-2">
            <Skeleton className="h-5 w-5 rounded" />
            <Skeleton className="h-5 w-40" />
          </div>
          <Skeleton className="h-5 w-16" />
        </div>
        <div className="mt-3 flex gap-3">
          <Skeleton className="h-3 w-20" />
          <Skeleton className="h-3 w-24" />
          <Skeleton className="h-3 w-20" />
        </div>
      </CardContent>
    </Card>
  );
}

export default function ContainersPage() {
  const [search, setSearch] = useState("");
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();

  const { data, isLoading, error } = useQuery({
    ...apiV1ContainersListOptions({ query: { limit: 100 } }),
    staleTime: 30 * 1000,
  });

  const containers = useMemo(() => {
    const list = (data as ContainerList | undefined)?.data ?? [];
    // Newest-first from the API; keep that order.
    return list;
  }, [data]);

  const deleteMutation = useMutation({
    ...apiV1ContainersDeleteMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "apiV1ContainersList" }] });
      toast({ title: "Container deleted", type: "success" });
    },
    onError: (err) => {
      toast({
        title: "Failed to delete container",
        description: formatApiError(err),
        type: "error",
      });
    },
  });

  const handleDelete = async (container: Container) => {
    const ok = await confirm({
      title: "Delete container?",
      message: `This permanently deletes ${
        container.name?.trim() || container.id
      } and all of its files. This cannot be undone.`,
      confirmLabel: "Delete",
      variant: "destructive",
    });
    if (ok) {
      deleteMutation.mutate({ path: { container_id: container.id } });
    }
  };

  const filtered = containers.filter((c) => {
    const q = search.trim().toLowerCase();
    if (!q) return true;
    return (
      c.id.toLowerCase().includes(q) ||
      (c.name?.toLowerCase().includes(q) ?? false) ||
      c.runtime.toLowerCase().includes(q)
    );
  });

  return (
    <div className="p-6 max-w-6xl mx-auto">
      <div className="mb-6">
        <h1 className="text-2xl font-semibold">Containers</h1>
        <p className="text-sm text-muted-foreground mt-1">
          Persistent sandboxes that back the shell tool in agent runs. Containers are created
          automatically when a response uses the shell tool, and reaped on their idle TTL.
        </p>
      </div>

      <div className="mb-6">
        <Input
          placeholder="Search containers..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="max-w-sm"
        />
      </div>

      {error && (
        <div className="rounded-md bg-destructive/10 px-4 py-3 text-sm text-destructive mb-6">
          Failed to load containers. Please try again.
        </div>
      )}

      {isLoading && (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <ContainerCardSkeleton key={i} />
          ))}
        </div>
      )}

      {!isLoading && containers.length === 0 && (
        <div className="text-center py-12">
          <Box className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
          <h2 className="text-lg font-medium mb-2">No containers yet</h2>
          <p className="text-sm text-muted-foreground max-w-md mx-auto">
            Containers appear here once an agent run uses the shell tool. Enable the agent tools in
            chat to create one.
          </p>
        </div>
      )}

      {!isLoading && containers.length > 0 && filtered.length === 0 && (
        <div className="text-center py-12">
          <Box className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
          <h2 className="text-lg font-medium mb-2">No matching containers</h2>
          <p className="text-sm text-muted-foreground">
            Try adjusting your search terms or{" "}
            <button onClick={() => setSearch("")} className="text-primary hover:underline">
              clear the search
            </button>
          </p>
        </div>
      )}

      {!isLoading && filtered.length > 0 && (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {filtered.map((c) => (
            <ContainerCard
              key={c.id}
              container={c}
              onDelete={handleDelete}
              deleting={deleteMutation.isPending}
            />
          ))}
        </div>
      )}
    </div>
  );
}
