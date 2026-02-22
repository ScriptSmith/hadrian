import { useState, useEffect, useRef } from "react";
import { FolderOpen, User } from "lucide-react";

import { Button } from "@/components/Button/Button";
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

export interface MoveToProjectModalProps {
  /** Whether the modal is open */
  open: boolean;
  /** Callback when modal is closed */
  onClose: () => void;
  /** The conversation being moved */
  conversation: {
    id: string;
    title: string;
    projectId?: string;
  };
  /** Callback to perform the move (receives project ID or null for personal, and project name if applicable) */
  onMove: (projectId: string | null, projectName?: string) => Promise<void>;
}

export function MoveToProjectModal({
  open,
  onClose,
  conversation,
  onMove,
}: MoveToProjectModalProps) {
  const { projects, isLoading: projectsLoading } = useUserProjects();
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [isMoving, setIsMoving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Reset selection when modal opens
  // Using a ref to track if we've initialized for this modal open
  const hasInitialized = useRef(false);

  useEffect(() => {
    if (open && !hasInitialized.current && !projectsLoading) {
      hasInitialized.current = true;
      // Default to "My Conversations" if currently in a project, otherwise first available project
      if (conversation.projectId) {
        setSelectedProjectId(null); // Moving out of project to personal
      } else if (projects.length > 0) {
        setSelectedProjectId(projects[0].id); // Moving to first project
      }
      setError(null);
    } else if (!open) {
      hasInitialized.current = false;
    }
  }, [open, conversation.projectId, projects, projectsLoading]);

  const handleMove = async () => {
    // Validate: can't move to the same location
    if (selectedProjectId === conversation.projectId) {
      setError("Conversation is already in this location");
      return;
    }
    // Validate: must select a destination if moving from personal
    if (!conversation.projectId && selectedProjectId === null) {
      setError("Please select a project");
      return;
    }

    setIsMoving(true);
    setError(null);
    try {
      const selectedProject = selectedProjectId
        ? projects.find((p) => p.id === selectedProjectId)
        : null;
      await onMove(selectedProjectId, selectedProject?.name);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to move conversation");
    } finally {
      setIsMoving(false);
    }
  };

  const handleClose = () => {
    if (!isMoving) {
      setError(null);
      onClose();
    }
  };

  // Determine what destinations are available
  const canMoveToPersonal = conversation.projectId != null; // Can only move to personal if currently in a project
  const availableProjects = projects.filter((p) => p.id !== conversation.projectId);

  return (
    <Modal open={open} onClose={handleClose} className="max-w-md">
      <ModalClose onClose={handleClose} />
      <ModalHeader>
        <ModalTitle className="flex items-center gap-2">
          <FolderOpen className="h-5 w-5" />
          Move Conversation
        </ModalTitle>
      </ModalHeader>

      <ModalContent>
        <div className="space-y-4">
          <p className="text-sm text-muted-foreground">Move &quot;{conversation.title}&quot; to:</p>

          {error && (
            <div className="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">
              {error}
            </div>
          )}

          {projectsLoading ? (
            <div className="flex items-center justify-center py-8">
              <div className="h-6 w-6 animate-spin rounded-full border-2 border-primary border-t-transparent" />
            </div>
          ) : (
            <div className="space-y-2" role="radiogroup" aria-label="Destination">
              {/* Option: My Conversations (only if currently in a project) */}
              {canMoveToPersonal && (
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
                    "flex cursor-pointer items-center gap-3 rounded-lg border p-3 transition-colors",
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
                  <User className="h-5 w-5 text-muted-foreground" />
                  <div className="flex-1">
                    <span className="font-medium">My Conversations</span>
                    <p className="text-xs text-muted-foreground">Personal (not shared)</p>
                  </div>
                </div>
              )}

              {/* Available projects */}
              {availableProjects.length === 0 && !canMoveToPersonal ? (
                <div className="rounded-lg border border-dashed p-4 text-center text-sm text-muted-foreground">
                  No projects available to move to.
                  <br />
                  Create a project first.
                </div>
              ) : (
                availableProjects.map((project) => (
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
                      "flex cursor-pointer items-center gap-3 rounded-lg border p-3 transition-colors",
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
                    <FolderOpen className="h-5 w-5 text-muted-foreground" />
                    <div className="flex-1">
                      <span className="font-medium">{project.name}</span>
                      <p className="text-xs text-muted-foreground">{project.org_name}</p>
                    </div>
                  </div>
                ))
              )}
            </div>
          )}
        </div>
      </ModalContent>

      <ModalFooter>
        <Button type="button" variant="ghost" onClick={handleClose} disabled={isMoving}>
          Cancel
        </Button>
        <Button
          type="button"
          onClick={handleMove}
          isLoading={isMoving}
          disabled={
            projectsLoading ||
            (availableProjects.length === 0 && !canMoveToPersonal) ||
            (selectedProjectId === conversation.projectId && selectedProjectId !== null)
          }
        >
          Move
        </Button>
      </ModalFooter>
    </Modal>
  );
}
