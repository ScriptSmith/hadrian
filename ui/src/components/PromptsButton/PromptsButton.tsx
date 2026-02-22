import { useState } from "react";
import { Loader2, Plus, Sparkles } from "lucide-react";

import { Button } from "@/components/Button/Button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/Popover/Popover";
import { PromptFormModal } from "@/components/PromptFormModal/PromptFormModal";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { useToast } from "@/components/Toast/Toast";
import { useUserPrompts } from "@/hooks/useUserPrompts";
import { cn } from "@/utils/cn";

interface PromptsButtonProps {
  /** Called when a prompt is selected */
  onApplyPrompt: (content: string) => void;
  /** Whether a system prompt is currently active (highlights the button) */
  hasActivePrompt?: boolean;
  /** Whether the button is disabled */
  disabled?: boolean;
}

export function PromptsButton({
  onApplyPrompt,
  hasActivePrompt = false,
  disabled = false,
}: PromptsButtonProps) {
  const [open, setOpen] = useState(false);
  const [createModalOpen, setCreateModalOpen] = useState(false);
  const { prompts, isLoading } = useUserPrompts();
  const { toast } = useToast();

  const handleSelect = (content: string) => {
    onApplyPrompt(content);
    setOpen(false);
  };

  return (
    <>
      <Popover open={open} onOpenChange={setOpen}>
        <Tooltip>
          <TooltipTrigger asChild>
            <PopoverTrigger asChild>
              <Button
                type="button"
                size="icon"
                variant="ghost"
                className={cn(
                  "h-8 w-8 shrink-0 rounded-lg",
                  hasActivePrompt ? "text-primary" : "text-muted-foreground hover:text-foreground"
                )}
                disabled={disabled}
                aria-label="Prompt templates"
              >
                <Sparkles className="h-4 w-4" />
              </Button>
            </PopoverTrigger>
          </TooltipTrigger>
          <TooltipContent side="top">Templates</TooltipContent>
        </Tooltip>

        <PopoverContent align="start" className="w-64 p-0">
          {/* Header */}
          <div className="flex items-center justify-between border-b px-3 py-2">
            <span className="text-sm font-medium">Templates</span>
            <Button
              variant="ghost"
              size="icon"
              className="h-6 w-6 text-muted-foreground hover:text-foreground"
              onClick={() => {
                setOpen(false);
                setCreateModalOpen(true);
              }}
              aria-label="Create new template"
            >
              <Plus className="h-3.5 w-3.5" />
            </Button>
          </div>

          {/* List */}
          <div className="max-h-60 overflow-y-auto scrollbar-thin p-1">
            {isLoading ? (
              <div className="flex items-center justify-center py-6">
                <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
              </div>
            ) : prompts.length === 0 ? (
              <div className="px-3 py-6 text-center">
                <Sparkles className="mx-auto mb-2 h-5 w-5 text-muted-foreground" />
                <p className="text-xs text-muted-foreground">No templates yet</p>
                <button
                  type="button"
                  className="mt-2 text-xs text-primary hover:underline"
                  onClick={() => {
                    setOpen(false);
                    setCreateModalOpen(true);
                  }}
                >
                  Create one
                </button>
              </div>
            ) : (
              <ul className="space-y-0.5">
                {prompts.map((prompt) => (
                  <li key={prompt.id}>
                    <button
                      type="button"
                      className={cn(
                        "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm",
                        "hover:bg-accent/50 transition-colors",
                        "text-foreground/80"
                      )}
                      onClick={() => handleSelect(prompt.content)}
                      title={prompt.description || "Apply this template"}
                    >
                      <Sparkles className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                      <span className="min-w-0 flex-1 truncate">{prompt.name}</span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </PopoverContent>
      </Popover>

      <PromptFormModal
        open={createModalOpen}
        onClose={() => setCreateModalOpen(false)}
        onSaved={() => {
          toast({ title: "Template created", type: "success" });
        }}
      />
    </>
  );
}
