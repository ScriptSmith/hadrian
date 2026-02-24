import { useState, useMemo } from "react";
import { Link } from "react-router-dom";
import { BookOpen, Plus, Calendar, FileText, HardDrive } from "lucide-react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";

import {
  organizationListOptions,
  vectorStoreListOptions,
  vectorStoreCreateMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import type { VectorStore, CreateVectorStore } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Badge } from "@/components/Badge/Badge";
import { Card, CardContent } from "@/components/Card/Card";
import { Input } from "@/components/Input/Input";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { VectorStoreFormModal } from "@/components/Admin";
import { useToast } from "@/components/Toast/Toast";
import { formatDateTime, formatBytes } from "@/utils/formatters";

function StatusBadge({ status }: { status: string }) {
  const variants: Record<string, "default" | "secondary" | "destructive" | "outline"> = {
    completed: "default",
    in_progress: "secondary",
    expired: "outline",
  };

  const labels: Record<string, string> = {
    completed: "Ready",
    in_progress: "Processing",
    expired: "Expired",
  };

  return <Badge variant={variants[status] || "outline"}>{labels[status] || status}</Badge>;
}

function KnowledgeBaseCard({ kb }: { kb: VectorStore }) {
  return (
    <Link to={`/admin/vector-stores/${kb.id}`} className="block">
      <Card className="h-full transition-colors hover:bg-muted/50">
        <CardContent className="p-4">
          <div className="flex items-start justify-between gap-2">
            <div className="flex items-center gap-2 min-w-0">
              <BookOpen className="h-5 w-5 text-muted-foreground shrink-0" />
              <p className="font-medium truncate">{kb.name}</p>
            </div>
            <StatusBadge status={kb.status} />
          </div>

          {kb.description && (
            <p className="mt-1 text-sm text-muted-foreground line-clamp-2">{kb.description}</p>
          )}

          <div className="mt-3 flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
            <span className="flex items-center gap-1">
              <FileText className="h-3 w-3" />
              {kb.file_counts.total} file{kb.file_counts.total !== 1 ? "s" : ""}
              {kb.file_counts.in_progress > 0 && (
                <Badge variant="secondary" className="ml-1 text-[10px] px-1 py-0">
                  {kb.file_counts.in_progress} processing
                </Badge>
              )}
            </span>
            <span className="flex items-center gap-1">
              <HardDrive className="h-3 w-3" />
              {formatBytes(kb.usage_bytes)}
            </span>
            <span className="flex items-center gap-1">
              <Calendar className="h-3 w-3" />
              {formatDateTime(kb.created_at)}
            </span>
          </div>
        </CardContent>
      </Card>
    </Link>
  );
}

function KnowledgeBaseCardSkeleton() {
  return (
    <Card>
      <CardContent className="p-4">
        <div className="flex items-start justify-between gap-2">
          <div className="flex items-center gap-2">
            <Skeleton className="h-5 w-5 rounded" />
            <Skeleton className="h-5 w-32" />
          </div>
          <Skeleton className="h-5 w-16" />
        </div>
        <Skeleton className="mt-2 h-4 w-full" />
        <div className="mt-3 flex gap-3">
          <Skeleton className="h-3 w-16" />
          <Skeleton className="h-3 w-16" />
          <Skeleton className="h-3 w-24" />
        </div>
      </CardContent>
    </Card>
  );
}

export default function KnowledgeBasesPage() {
  const [search, setSearch] = useState("");
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const { toast } = useToast();
  const queryClient = useQueryClient();

  // Fetch all accessible vector stores (single query)
  const {
    data: vectorStoresData,
    isLoading: vectorStoresLoading,
    error: vectorStoresError,
  } = useQuery({
    ...vectorStoreListOptions({
      query: {
        limit: 100,
      },
    }),
    staleTime: 5 * 60 * 1000,
  });

  // Fetch organizations (still needed for the create modal)
  const { data: orgsData, isLoading: orgsLoading } = useQuery(organizationListOptions());

  const organizations = useMemo(() => orgsData?.data ?? [], [orgsData?.data]);

  // Sort knowledge bases by name
  const knowledgeBases = useMemo(() => {
    const stores = vectorStoresData?.data ?? [];
    return [...stores].sort((a, b) => a.name.localeCompare(b.name));
  }, [vectorStoresData?.data]);

  const isLoading = vectorStoresLoading;
  const error = vectorStoresError;

  const filteredKnowledgeBases = knowledgeBases.filter(
    (kb) =>
      kb.name.toLowerCase().includes(search.toLowerCase()) ||
      (kb.description?.toLowerCase().includes(search.toLowerCase()) ?? false)
  );

  const createMutation = useMutation({
    ...vectorStoreCreateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "vectorStoreList" }] });
      setIsCreateModalOpen(false);
      toast({ title: "Knowledge base created", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to create knowledge base",
        description: String(error),
        type: "error",
      });
    },
  });

  const handleCreateSubmit = (data: CreateVectorStore) => {
    createMutation.mutate({ body: data });
  };

  return (
    <div className="p-6 max-w-6xl mx-auto">
      {/* Header */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between mb-6">
        <div>
          <h1 className="text-2xl font-semibold">Knowledge Bases</h1>
          <p className="text-sm text-muted-foreground mt-1">
            Store and search documents for RAG (Retrieval Augmented Generation)
          </p>
        </div>
        <Button
          onClick={() => setIsCreateModalOpen(true)}
          disabled={orgsLoading || organizations.length === 0}
        >
          <Plus className="h-4 w-4 mr-2" />
          New Knowledge Base
        </Button>
      </div>

      {/* Search */}
      <div className="mb-6">
        <Input
          placeholder="Search knowledge bases..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="max-w-sm"
        />
      </div>

      {/* Error state */}
      {error && (
        <div className="rounded-md bg-destructive/10 px-4 py-3 text-sm text-destructive mb-6">
          Failed to load knowledge bases. Please try again.
        </div>
      )}

      {/* Loading state */}
      {isLoading && (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <KnowledgeBaseCardSkeleton key={i} />
          ))}
        </div>
      )}

      {/* Empty state - no knowledge bases */}
      {!isLoading && knowledgeBases.length === 0 && (
        <div className="text-center py-12">
          <BookOpen className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
          <h2 className="text-lg font-medium mb-2">No knowledge bases yet</h2>
          <p className="text-sm text-muted-foreground max-w-md mx-auto mb-4">
            Create a knowledge base to store documents and enable semantic search for your AI
            assistants.
          </p>
          <Button onClick={() => setIsCreateModalOpen(true)}>
            <Plus className="h-4 w-4 mr-2" />
            Create Knowledge Base
          </Button>
        </div>
      )}

      {/* Empty state - no search results */}
      {!isLoading && knowledgeBases.length > 0 && filteredKnowledgeBases.length === 0 && (
        <div className="text-center py-12">
          <BookOpen className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
          <h2 className="text-lg font-medium mb-2">No matching knowledge bases</h2>
          <p className="text-sm text-muted-foreground">
            Try adjusting your search terms or{" "}
            <button onClick={() => setSearch("")} className="text-primary hover:underline">
              clear the search
            </button>
          </p>
        </div>
      )}

      {/* Knowledge bases grid */}
      {!isLoading && filteredKnowledgeBases.length > 0 && (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {filteredKnowledgeBases.map((kb) => (
            <KnowledgeBaseCard key={kb.id} kb={kb} />
          ))}
        </div>
      )}

      {/* Create modal */}
      <VectorStoreFormModal
        isOpen={isCreateModalOpen}
        onClose={() => setIsCreateModalOpen(false)}
        onCreateSubmit={handleCreateSubmit}
        onEditSubmit={() => {}}
        isLoading={createMutation.isPending}
        editingStore={null}
        organizations={organizations}
      />
    </div>
  );
}
