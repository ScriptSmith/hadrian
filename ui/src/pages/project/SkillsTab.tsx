import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { type ColumnDef } from "@tanstack/react-table";
import { Plus } from "lucide-react";

import {
  skillListByProjectOptions,
  skillDeleteMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import type { Skill } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { DataTable } from "@/components/DataTable/DataTable";
import { SkillFormModal } from "@/components/Admin";
import { useToast } from "@/components/Toast/Toast";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";
import { createSkillColumns } from "@/pages/admin/skillColumns";

import { formatApiError } from "@/utils/formatApiError";
interface SkillsTabProps {
  orgSlug: string;
  projectSlug: string;
  projectId: string;
}

export function SkillsTab({ orgSlug, projectSlug, projectId }: SkillsTabProps) {
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingSkill, setEditingSkill] = useState<Skill | null>(null);

  const { data: skillsData, isLoading } = useQuery(
    skillListByProjectOptions({
      path: { org_slug: orgSlug, project_slug: projectSlug },
    })
  );

  const deleteSkillMutation = useMutation({
    ...skillDeleteMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "skillListByProject" }] });
      toast({ title: "Skill deleted", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to delete skill", description: formatApiError(error), type: "error" });
    },
  });

  const handleEdit = (skill: Skill) => {
    setEditingSkill(skill);
    setIsModalOpen(true);
  };

  const handleDelete = async (skill: Skill) => {
    const confirmed = await confirm({
      title: "Delete Skill",
      message: `Are you sure you want to delete "${skill.name}"? This action cannot be undone.`,
      confirmLabel: "Delete",
      variant: "destructive",
    });
    if (confirmed) {
      deleteSkillMutation.mutate({ path: { id: skill.id } });
    }
  };

  const columns = createSkillColumns(handleEdit, handleDelete);

  return (
    <>
      <Card>
        <CardHeader className="flex-row items-center justify-between">
          <CardTitle>Skills</CardTitle>
          <Button
            size="sm"
            onClick={() => {
              setEditingSkill(null);
              setIsModalOpen(true);
            }}
          >
            <Plus className="mr-2 h-4 w-4" />
            New Skill
          </Button>
        </CardHeader>
        <CardContent>
          <DataTable
            columns={columns as ColumnDef<Skill>[]}
            data={skillsData?.data || []}
            isLoading={isLoading}
            emptyMessage="No skills in this project."
            searchColumn="name"
            searchPlaceholder="Search skills..."
          />
        </CardContent>
      </Card>

      <SkillFormModal
        open={isModalOpen}
        onClose={() => {
          setIsModalOpen(false);
          setEditingSkill(null);
        }}
        editingSkill={editingSkill}
        ownerOverride={{ type: "project", project_id: projectId }}
        onSaved={() => {
          queryClient.invalidateQueries({ queryKey: [{ _id: "skillListByProject" }] });
          toast({
            title: editingSkill ? "Skill updated" : "Skill created",
            type: "success",
          });
        }}
      />
    </>
  );
}
