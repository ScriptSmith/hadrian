import { useState } from "react";
import { ClipboardPenLine, Loader2, Plus, Trash2 } from "lucide-react";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import type { Template } from "@/api/generated/types.gen";
import { templateDelete } from "@/api/generated/sdk.gen";
import { Button } from "@/components/Button/Button";
import {
  Modal,
  ModalClose,
  ModalContent,
  ModalFooter,
  ModalHeader,
  ModalTitle,
} from "@/components/Modal/Modal";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/Popover/Popover";
import { PromptFormModal } from "@/components/PromptFormModal/PromptFormModal";
import { TemplateVariableForm } from "@/components/TemplateVariableForm/TemplateVariableForm";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { useToast } from "@/components/Toast/Toast";
import { useUserTemplates } from "@/hooks/useUserPrompts";
import {
  parseVariables,
  substituteVariables,
  validateVariableValues,
} from "@/lib/templateVariables";

interface TemplatesButtonProps {
  /** Called when a template is selected */
  onApplyTemplate: (content: string) => void;
  /** Whether the button is disabled */
  disabled?: boolean;
}

export function TemplatesButton({ onApplyTemplate, disabled = false }: TemplatesButtonProps) {
  const [open, setOpen] = useState(false);
  const [createModalOpen, setCreateModalOpen] = useState(false);
  const [variableTemplate, setVariableTemplate] = useState<Template | null>(null);
  const [variableValues, setVariableValues] = useState<Record<string, string>>({});
  const [variableErrors, setVariableErrors] = useState<Record<string, string>>({});
  const { templates, isLoading, hasMore } = useUserTemplates();
  const { toast } = useToast();
  const queryClient = useQueryClient();

  const deleteMutation = useMutation({
    mutationFn: async (id: string) => {
      const response = await templateDelete({ path: { id } });
      if (response.error) throw new Error("Failed to delete template");
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "templateListByOrg" }] });
      queryClient.invalidateQueries({ queryKey: [{ _id: "templateListByUser" }] });
      toast({ title: "Template deleted", type: "success" });
    },
    onError: () => {
      toast({ title: "Failed to delete template", type: "error" });
    },
  });

  const handleSelect = (template: Template) => {
    setOpen(false);
    const vars = parseVariables(template.metadata);
    if (vars.length > 0) {
      const defaults: Record<string, string> = {};
      for (const v of vars) {
        if (v.default) defaults[v.name] = v.default;
      }
      setVariableValues(defaults);
      setVariableErrors({});
      setVariableTemplate(template);
    } else {
      onApplyTemplate(template.content);
    }
  };

  const handleVariableSubmit = () => {
    if (!variableTemplate) return;
    const vars = parseVariables(variableTemplate.metadata);
    const errors = validateVariableValues(vars, variableValues);
    if (Object.keys(errors).length > 0) {
      setVariableErrors(errors);
      return;
    }
    const content = substituteVariables(variableTemplate.content, variableValues);
    setVariableTemplate(null);
    onApplyTemplate(content);
  };

  const handleDelete = (e: React.MouseEvent, template: Template) => {
    e.stopPropagation();
    deleteMutation.mutate(template.id);
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
                className="h-8 w-8 shrink-0 rounded-lg text-muted-foreground hover:text-foreground"
                disabled={disabled}
                aria-label="Templates"
              >
                <ClipboardPenLine className="h-4 w-4" />
              </Button>
            </PopoverTrigger>
          </TooltipTrigger>
          <TooltipContent side="top">Templates</TooltipContent>
        </Tooltip>

        <PopoverContent align="start" className="w-72 p-0">
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
            ) : templates.length === 0 ? (
              <div className="px-3 py-6 text-center">
                <ClipboardPenLine className="mx-auto mb-2 h-5 w-5 text-muted-foreground" />
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
              <>
                <ul className="space-y-0.5">
                  {templates.map((template) => (
                    <li key={template.id} className="group relative">
                      <button
                        type="button"
                        className="flex w-full items-start gap-2 rounded-md px-2 py-1.5 pr-8 text-left text-sm hover:bg-accent/50 transition-colors text-foreground/80"
                        onClick={() => handleSelect(template)}
                        title={template.description || "Apply this template"}
                      >
                        <ClipboardPenLine className="mt-0.5 h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                        <div className="min-w-0 flex-1">
                          <span className="block truncate">{template.name}</span>
                          {template.description && (
                            <span className="text-xs text-muted-foreground line-clamp-1">
                              {template.description}
                            </span>
                          )}
                        </div>
                      </button>
                      <button
                        type="button"
                        className="absolute right-1.5 top-1.5 hidden rounded p-0.5 text-muted-foreground hover:bg-destructive/10 hover:text-destructive group-hover:block"
                        onClick={(e) => handleDelete(e, template)}
                        aria-label={`Delete template: ${template.name}`}
                      >
                        <Trash2 className="h-3 w-3" />
                      </button>
                    </li>
                  ))}
                </ul>
                {hasMore && (
                  <p className="px-2 py-1.5 text-center text-xs text-muted-foreground">
                    Showing first 50 templates
                  </p>
                )}
              </>
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

      {/* Variable fill modal */}
      <Modal
        open={!!variableTemplate}
        onClose={() => setVariableTemplate(null)}
        className="max-w-md"
      >
        <ModalClose onClose={() => setVariableTemplate(null)} />
        <ModalHeader>
          <ModalTitle className="flex items-center gap-2">
            <ClipboardPenLine className="h-5 w-5" />
            {variableTemplate?.name}
          </ModalTitle>
        </ModalHeader>
        <ModalContent>
          {variableTemplate && (
            <>
              {variableTemplate.description && (
                <p className="mb-3 text-sm text-muted-foreground">{variableTemplate.description}</p>
              )}
              <TemplateVariableForm
                variables={parseVariables(variableTemplate.metadata)}
                values={variableValues}
                onChange={setVariableValues}
                errors={variableErrors}
              />
              {/* Preview */}
              <div className="mt-4">
                <p className="mb-1 text-xs font-medium text-muted-foreground">Preview</p>
                <pre className="max-h-32 overflow-y-auto rounded-md border bg-muted/50 p-2 text-xs whitespace-pre-wrap">
                  {substituteVariables(variableTemplate.content, variableValues)}
                </pre>
              </div>
            </>
          )}
        </ModalContent>
        <ModalFooter>
          <Button variant="ghost" onClick={() => setVariableTemplate(null)}>
            Cancel
          </Button>
          <Button onClick={handleVariableSubmit}>Apply Template</Button>
        </ModalFooter>
      </Modal>
    </>
  );
}
