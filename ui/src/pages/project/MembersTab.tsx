import { useState } from "react";
import { createColumnHelper, type ColumnDef } from "@tanstack/react-table";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Trash2, Plus } from "lucide-react";

import {
  projectMemberListOptions,
  projectMemberAddMutation,
  projectMemberRemoveMutation,
  userListOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type { User } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { DataTable } from "@/components/DataTable/DataTable";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { AddMemberModal } from "@/components/Admin";
import { useToast } from "@/components/Toast/Toast";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";

const columnHelper = createColumnHelper<User>();

interface MembersTabProps {
  orgSlug: string;
  projectSlug: string;
}

export function MembersTab({ orgSlug, projectSlug }: MembersTabProps) {
  const { toast } = useToast();
  const queryClient = useQueryClient();
  const confirm = useConfirm();
  const [isAddModalOpen, setIsAddModalOpen] = useState(false);

  const { data: members, isLoading } = useQuery(
    projectMemberListOptions({
      path: { org_slug: orgSlug, project_slug: projectSlug },
    })
  );

  const { data: allUsers } = useQuery({
    ...userListOptions(),
    enabled: isAddModalOpen,
  });

  const addMutation = useMutation({
    ...projectMemberAddMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "projectMemberList" }] });
      setIsAddModalOpen(false);
      toast({ title: "Member added", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to add member", description: String(error), type: "error" });
    },
  });

  const removeMutation = useMutation({
    ...projectMemberRemoveMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "projectMemberList" }] });
      toast({ title: "Member removed", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to remove member", description: String(error), type: "error" });
    },
  });

  const handleRemoveMember = async (userId: string, userName?: string) => {
    const confirmed = await confirm({
      title: "Remove Member",
      message: `Are you sure you want to remove ${userName || "this member"} from the project?`,
      confirmLabel: "Remove",
      variant: "destructive",
    });
    if (confirmed) {
      removeMutation.mutate({
        path: { org_slug: orgSlug, project_slug: projectSlug, user_id: userId },
      });
    }
  };

  const columns = [
    columnHelper.accessor("name", {
      header: "Name",
      cell: (info) => info.getValue() || "-",
    }),
    columnHelper.accessor("email", {
      header: "Email",
      cell: (info) => info.getValue() || "-",
    }),
    columnHelper.accessor("external_id", {
      header: "External ID",
      cell: (info) => <CodeBadge>{info.getValue()}</CodeBadge>,
    }),
    columnHelper.display({
      id: "actions",
      header: () => <span className="sr-only">Actions</span>,
      cell: ({ row }) => (
        <Button
          variant="ghost"
          size="sm"
          className="text-destructive"
          aria-label="Remove member"
          onClick={() =>
            handleRemoveMember(
              row.original.id,
              row.original.name || row.original.email || row.original.external_id
            )
          }
        >
          <Trash2 className="h-4 w-4" />
        </Button>
      ),
    }),
  ];

  const availableUsers = allUsers?.data?.filter(
    (user) => !members?.data?.some((member) => member.id === user.id)
  );

  return (
    <>
      <Card role="tabpanel" id="tabpanel-members" aria-labelledby="tab-members">
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle>Members</CardTitle>
          <Button size="sm" onClick={() => setIsAddModalOpen(true)}>
            <Plus className="mr-2 h-4 w-4" />
            Add Member
          </Button>
        </CardHeader>
        <CardContent>
          <DataTable
            columns={columns as ColumnDef<User>[]}
            data={members?.data || []}
            isLoading={isLoading}
            emptyMessage="No members in this project."
            searchColumn="name"
            searchPlaceholder="Search members..."
          />
        </CardContent>
      </Card>

      <AddMemberModal
        open={isAddModalOpen}
        onClose={() => setIsAddModalOpen(false)}
        onSubmit={(userId) =>
          addMutation.mutate({
            path: { org_slug: orgSlug, project_slug: projectSlug },
            body: { user_id: userId },
          })
        }
        availableUsers={availableUsers || []}
        isLoading={addMutation.isPending}
        emptyMessage="All users are already members of this project."
      />
    </>
  );
}
