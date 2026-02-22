import { useState, useEffect, useRef } from "react";
import { Check, FolderOpen, GitFork, User } from "lucide-react";

import { Button } from "@/components/Button/Button";
import type { Conversation } from "@/components/chat-types";
import { Input } from "@/components/Input/Input";
import { Label } from "@/components/Label/Label";
import {
  Modal,
  ModalClose,
  ModalHeader,
  ModalTitle,
  ModalContent,
  ModalFooter,
} from "@/components/Modal/Modal";
import { useUserProjects } from "@/hooks/useUserProjects";
import { cn } from "@/utils/cn";
import { getModelDisplayName } from "@/utils/modelNames";

export interface ForkConversationResult {
  /** Custom title for the forked conversation */
  title: string;
  /** Models to include in the fork (subset of original) */
  models: string[];
  /** Target project ID (null for personal) */
  projectId: string | null;
  /** Target project name (for display) */
  projectName?: string;
}

export interface ForkConversationModalProps {
  /** Whether the modal is open */
  open: boolean;
  /** Callback when modal is closed */
  onClose: () => void;
  /** The conversation being forked */
  conversation: Conversation;
  /** Optional message ID to fork from (for partial forks) */
  upToMessageId?: string;
  /** Callback to perform the fork */
  onFork: (result: ForkConversationResult) => void;
}

export function ForkConversationModal({
  open,
  onClose,
  conversation,
  upToMessageId,
  onFork,
}: ForkConversationModalProps) {
  const { projects, isLoading: projectsLoading } = useUserProjects();
  const [title, setTitle] = useState("");
  const [selectedModels, setSelectedModels] = useState<string[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Track initialization
  const hasInitialized = useRef(false);

  // Get unique models from the conversation
  const conversationModels = conversation.models;

  // Reset state when modal opens
  useEffect(() => {
    if (open && !hasInitialized.current) {
      hasInitialized.current = true;
      // Default title with (fork) suffix
      const baseTitle = conversation.title.replace(/\s*\(fork\)$/i, "");
      setTitle(`${baseTitle} (fork)`);
      // Select all models by default
      setSelectedModels([...conversationModels]);
      // Default to same project as source, or personal if none
      setSelectedProjectId(conversation.projectId ?? null);
      setError(null);
    } else if (!open) {
      hasInitialized.current = false;
    }
  }, [open, conversation.title, conversation.projectId, conversationModels]);

  const handleModelToggle = (modelId: string) => {
    setSelectedModels((prev) =>
      prev.includes(modelId) ? prev.filter((m) => m !== modelId) : [...prev, modelId]
    );
  };

  const handleFork = () => {
    // Validate
    if (!title.trim()) {
      setError("Please enter a title");
      return;
    }
    if (selectedModels.length === 0) {
      setError("Please select at least one model");
      return;
    }

    const selectedProject = selectedProjectId
      ? projects.find((p) => p.id === selectedProjectId)
      : null;

    onFork({
      title: title.trim(),
      models: selectedModels,
      projectId: selectedProjectId,
      projectName: selectedProject?.name,
    });
    onClose();
  };

  const handleClose = () => {
    setError(null);
    onClose();
  };

  // Determine message count for partial fork info
  const messageCount = upToMessageId
    ? conversation.messages.findIndex((m) => m.id === upToMessageId) + 1
    : conversation.messages.length;

  return (
    <Modal open={open} onClose={handleClose} className="max-w-md">
      <ModalClose onClose={handleClose} />
      <ModalHeader>
        <ModalTitle className="flex items-center gap-2">
          <GitFork className="h-5 w-5" />
          Fork Conversation
        </ModalTitle>
      </ModalHeader>

      <ModalContent>
        <div className="space-y-5">
          {/* Info about what's being forked */}
          <p className="text-sm text-muted-foreground">
            {upToMessageId ? (
              <>
                Fork first {messageCount} message{messageCount !== 1 ? "s" : ""} from &quot;
                {conversation.title}&quot;
              </>
            ) : (
              <>
                Fork all {messageCount} message{messageCount !== 1 ? "s" : ""} from &quot;
                {conversation.title}&quot;
              </>
            )}
          </p>

          {error && (
            <div className="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">
              {error}
            </div>
          )}

          {/* Title input */}
          <div className="space-y-2">
            <Label htmlFor="fork-title">Title</Label>
            <Input
              id="fork-title"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="Enter conversation title"
            />
          </div>

          {/* Model selection */}
          {conversationModels.length > 1 && (
            <div className="space-y-2">
              <Label>Models to include</Label>
              <div className="space-y-1.5 rounded-lg border p-3">
                {conversationModels.map((modelId) => (
                  <div
                    key={modelId}
                    role="checkbox"
                    aria-checked={selectedModels.includes(modelId)}
                    tabIndex={0}
                    onClick={() => handleModelToggle(modelId)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        handleModelToggle(modelId);
                      }
                    }}
                    className={cn(
                      "flex cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 transition-colors",
                      "hover:bg-accent/50"
                    )}
                  >
                    <div
                      className={cn(
                        "flex h-4 w-4 items-center justify-center rounded border transition-colors",
                        selectedModels.includes(modelId)
                          ? "border-primary bg-primary text-primary-foreground"
                          : "border-muted-foreground"
                      )}
                    >
                      {selectedModels.includes(modelId) && <Check className="h-3 w-3" />}
                    </div>
                    <span className="text-sm">{getModelDisplayName(modelId)}</span>
                  </div>
                ))}
              </div>
              <p className="text-xs text-muted-foreground">
                Deselecting models will remove their responses from the fork.
              </p>
            </div>
          )}

          {/* Project selection */}
          <div className="space-y-2">
            <Label>Destination</Label>
            {projectsLoading ? (
              <div className="flex items-center justify-center py-4">
                <div className="h-5 w-5 animate-spin rounded-full border-2 border-primary border-t-transparent" />
              </div>
            ) : (
              <div className="space-y-1.5" role="radiogroup" aria-label="Destination">
                {/* Personal option */}
                <div
                  role="radio"
                  aria-checked={selectedProjectId === null}
                  tabIndex={0}
                  onClick={() => setSelectedProjectId(null)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      setSelectedProjectId(null);
                    }
                  }}
                  className={cn(
                    "flex cursor-pointer items-center gap-3 rounded-lg border p-2.5 transition-colors",
                    selectedProjectId === null
                      ? "border-primary bg-primary/5"
                      : "border-border hover:bg-accent/50"
                  )}
                >
                  <div
                    className={cn(
                      "h-4 w-4 rounded-full border-2 flex items-center justify-center",
                      selectedProjectId === null ? "border-primary" : "border-muted-foreground"
                    )}
                  >
                    {selectedProjectId === null && (
                      <div className="h-2 w-2 rounded-full bg-primary" />
                    )}
                  </div>
                  <User className="h-4 w-4 text-muted-foreground" />
                  <div className="flex-1">
                    <span className="text-sm font-medium">My Conversations</span>
                  </div>
                </div>

                {/* Project options */}
                {projects.map((project) => (
                  <div
                    key={project.id}
                    role="radio"
                    aria-checked={selectedProjectId === project.id}
                    tabIndex={0}
                    onClick={() => setSelectedProjectId(project.id)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        setSelectedProjectId(project.id);
                      }
                    }}
                    className={cn(
                      "flex cursor-pointer items-center gap-3 rounded-lg border p-2.5 transition-colors",
                      selectedProjectId === project.id
                        ? "border-primary bg-primary/5"
                        : "border-border hover:bg-accent/50"
                    )}
                  >
                    <div
                      className={cn(
                        "h-4 w-4 rounded-full border-2 flex items-center justify-center",
                        selectedProjectId === project.id
                          ? "border-primary"
                          : "border-muted-foreground"
                      )}
                    >
                      {selectedProjectId === project.id && (
                        <div className="h-2 w-2 rounded-full bg-primary" />
                      )}
                    </div>
                    <FolderOpen className="h-4 w-4 text-muted-foreground" />
                    <div className="flex-1 min-w-0">
                      <span className="text-sm font-medium truncate block">{project.name}</span>
                      <span className="text-xs text-muted-foreground truncate block">
                        {project.org_name}
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </ModalContent>

      <ModalFooter>
        <Button type="button" variant="ghost" onClick={handleClose}>
          Cancel
        </Button>
        <Button type="button" onClick={handleFork} disabled={projectsLoading}>
          Fork
        </Button>
      </ModalFooter>
    </Modal>
  );
}
