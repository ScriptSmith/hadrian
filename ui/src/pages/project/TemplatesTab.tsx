import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { type ColumnDef } from "@tanstack/react-table";
import { Plus } from "lucide-react";

import {
  templateListByProjectOptions,
  templateDeleteMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import type { Template } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { DataTable } from "@/components/DataTable/DataTable";
import { AdminPromptFormModal } from "@/components/Admin";
import { useToast } from "@/components/Toast/Toast";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";
import { createTemplateColumns } from "@/pages/admin/promptColumns";

interface TemplatesTabProps {
  orgSlug: string;
  projectSlug: string;
  projectId: string;
}

export function TemplatesTab({ orgSlug, projectSlug, projectId }: TemplatesTabProps) {
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingTemplate, setEditingTemplate] = useState<Template | null>(null);

  const { data: templatesData, isLoading } = useQuery(
    templateListByProjectOptions({
      path: { org_slug: orgSlug, project_slug: projectSlug },
    })
  );

  const deleteTemplateMutation = useMutation({
    ...templateDeleteMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "templateListByProject" }] });
      toast({ title: "Template deleted", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to delete template", description: String(error), type: "error" });
    },
  });

  const handleEdit = (template: Template) => {
    setEditingTemplate(template);
    setIsModalOpen(true);
  };

  const handleDelete = async (template: Template) => {
    const confirmed = await confirm({
      title: "Delete Template",
      message: `Are you sure you want to delete "${template.name}"? This action cannot be undone.`,
      confirmLabel: "Delete",
      variant: "destructive",
    });
    if (confirmed) {
      deleteTemplateMutation.mutate({ path: { id: template.id } });
    }
  };

  const columns = createTemplateColumns(handleEdit, handleDelete);

  return (
    <>
      <Card>
        <CardHeader className="flex-row items-center justify-between">
          <CardTitle>Templates</CardTitle>
          <Button
            size="sm"
            onClick={() => {
              setEditingTemplate(null);
              setIsModalOpen(true);
            }}
          >
            <Plus className="mr-2 h-4 w-4" />
            New Template
          </Button>
        </CardHeader>
        <CardContent>
          <DataTable
            columns={columns as ColumnDef<Template>[]}
            data={templatesData?.data || []}
            isLoading={isLoading}
            emptyMessage="No templates in this project."
            searchColumn="name"
            searchPlaceholder="Search templates..."
          />
        </CardContent>
      </Card>

      <AdminPromptFormModal
        open={isModalOpen}
        onClose={() => {
          setIsModalOpen(false);
          setEditingTemplate(null);
        }}
        editingPrompt={editingTemplate}
        ownerOverride={{ type: "project", project_id: projectId }}
        onSaved={() => {
          queryClient.invalidateQueries({ queryKey: [{ _id: "templateListByProject" }] });
          toast({
            title: editingTemplate ? "Template updated" : "Template created",
            type: "success",
          });
        }}
      />
    </>
  );
}
