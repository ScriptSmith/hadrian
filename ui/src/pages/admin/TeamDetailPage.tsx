import { zodResolver } from "@hookform/resolvers/zod";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { createColumnHelper, type ColumnDef } from "@tanstack/react-table";
import { useParams, useNavigate } from "react-router-dom";
import { ArrowLeft, Users, Plus, Trash2, BarChart3 } from "lucide-react";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";

import {
  teamGetOptions,
  teamUpdateMutation,
  teamMemberListOptions,
  teamMemberAddMutation,
  teamMemberRemoveMutation,
  userListOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type { TeamMember } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import { DataTable } from "@/components/DataTable/DataTable";
import { FormField } from "@/components/FormField/FormField";
import { Input } from "@/components/Input/Input";
import { Modal, ModalHeader, ModalContent, ModalFooter } from "@/components/Modal/Modal";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { Badge } from "@/components/Badge/Badge";
import { useToast } from "@/components/Toast/Toast";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";
import { DetailPageHeader, TabNavigation, AddMemberModal, type Tab } from "@/components/Admin";
import { formatDateTime } from "@/utils/formatters";
import UsageDashboard from "@/components/UsageDashboard/UsageDashboard";

type TabId = "members" | "usage";

const tabs: Tab<TabId>[] = [
  { id: "members", label: "Members", icon: <Users className="h-4 w-4" /> },
  { id: "usage", label: "Usage", icon: <BarChart3 className="h-4 w-4" /> },
];

const memberColumnHelper = createColumnHelper<TeamMember>();

const editTeamSchema = z.object({
  name: z.string().min(1, "Name is required"),
});

type EditTeamForm = z.infer<typeof editTeamSchema>;

export default function TeamDetailPage() {
  const { orgSlug, teamSlug } = useParams<{ orgSlug: string; teamSlug: string }>();
  const navigate = useNavigate();
  const { toast } = useToast();
  const queryClient = useQueryClient();
  const confirm = useConfirm();

  const [activeTab, setActiveTab] = useState<TabId>("members");
  const [isEditModalOpen, setIsEditModalOpen] = useState(false);
  const [isAddMemberModalOpen, setIsAddMemberModalOpen] = useState(false);

  const editForm = useForm<EditTeamForm>({
    resolver: zodResolver(editTeamSchema),
    defaultValues: { name: "" },
  });

  // Fetch team details
  const {
    data: team,
    isLoading: teamLoading,
    error: teamError,
  } = useQuery(teamGetOptions({ path: { org_slug: orgSlug!, team_slug: teamSlug! } }));

  // Fetch members
  const { data: members, isLoading: membersLoading } = useQuery({
    ...teamMemberListOptions({
      path: { org_slug: orgSlug!, team_slug: teamSlug! },
    }),
    enabled: activeTab === "members",
  });

  // Fetch all users for member selection
  const { data: allUsers } = useQuery({
    ...userListOptions(),
    enabled: isAddMemberModalOpen,
  });

  // Update mutation
  const updateMutation = useMutation({
    ...teamUpdateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "teamGet" }] });
      setIsEditModalOpen(false);
      toast({ title: "Team updated", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to update team", description: String(error), type: "error" });
    },
  });

  // Add member mutation
  const addMemberMutation = useMutation({
    ...teamMemberAddMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "teamMemberList" }] });
      setIsAddMemberModalOpen(false);
      toast({ title: "Member added", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to add member", description: String(error), type: "error" });
    },
  });

  // Remove member mutation
  const removeMemberMutation = useMutation({
    ...teamMemberRemoveMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "teamMemberList" }] });
      toast({ title: "Member removed", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to remove member", description: String(error), type: "error" });
    },
  });

  const onEditSubmit = (data: EditTeamForm) => {
    updateMutation.mutate({
      path: { org_slug: orgSlug!, team_slug: teamSlug! },
      body: { name: data.name },
    });
  };

  const handleAddMember = (userId: string) => {
    addMemberMutation.mutate({
      path: { org_slug: orgSlug!, team_slug: teamSlug! },
      body: { user_id: userId },
    });
  };

  const handleRemoveMember = async (userId: string, userName?: string) => {
    const confirmed = await confirm({
      title: "Remove Member",
      message: `Are you sure you want to remove ${userName || "this member"} from the team?`,
      confirmLabel: "Remove",
      variant: "destructive",
    });
    if (confirmed) {
      removeMemberMutation.mutate({
        path: { org_slug: orgSlug!, team_slug: teamSlug!, user_id: userId },
      });
    }
  };

  // Column definitions
  const memberColumns = [
    memberColumnHelper.accessor("name", {
      header: "Name",
      cell: (info) => info.getValue() || "-",
    }),
    memberColumnHelper.accessor("email", {
      header: "Email",
      cell: (info) => info.getValue() || "-",
    }),
    memberColumnHelper.accessor("external_id", {
      header: "External ID",
      cell: (info) => <CodeBadge>{info.getValue()}</CodeBadge>,
    }),
    memberColumnHelper.accessor("role", {
      header: "Role",
      cell: (info) => <Badge variant="secondary">{info.getValue()}</Badge>,
    }),
    memberColumnHelper.accessor("joined_at", {
      header: "Joined",
      cell: (info) => formatDateTime(info.getValue()),
    }),
    memberColumnHelper.display({
      id: "actions",
      cell: ({ row }) => (
        <Button
          variant="ghost"
          size="sm"
          className="text-destructive"
          aria-label="Remove member"
          onClick={() =>
            handleRemoveMember(
              row.original.user_id,
              row.original.name || row.original.email || row.original.external_id
            )
          }
        >
          <Trash2 className="h-4 w-4" />
        </Button>
      ),
    }),
  ];

  if (teamLoading) {
    return (
      <div className="space-y-6 p-6">
        <Skeleton className="h-8 w-64" />
        <Skeleton className="h-32 w-full" />
      </div>
    );
  }

  if (teamError || !team) {
    return (
      <div className="p-6">
        <div className="py-12 text-center text-destructive">
          Team not found or failed to load.
          <br />
          <Button variant="ghost" onClick={() => navigate(`/admin/teams`)} className="mt-4">
            <ArrowLeft className="mr-2 h-4 w-4" />
            Back to Teams
          </Button>
        </div>
      </div>
    );
  }

  // Filter out users that are already members
  const availableUsers = allUsers?.data?.filter(
    (user) => !members?.data?.some((member) => member.user_id === user.id)
  );

  return (
    <div className="space-y-6 p-6">
      <DetailPageHeader
        title={team.name}
        slug={team.slug}
        createdAt={team.created_at}
        onBack={() => navigate(`/admin/teams`)}
        onEdit={() => {
          editForm.reset({ name: team.name });
          setIsEditModalOpen(true);
        }}
      />

      <TabNavigation tabs={tabs} activeTab={activeTab} onTabChange={setActiveTab} />

      {/* Tab Content */}
      <Card role="tabpanel" id={`tabpanel-${activeTab}`} aria-labelledby={`tab-${activeTab}`}>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle>{tabs.find((t) => t.id === activeTab)?.label}</CardTitle>
          {activeTab === "members" && (
            <Button size="sm" onClick={() => setIsAddMemberModalOpen(true)}>
              <Plus className="mr-2 h-4 w-4" />
              Add Member
            </Button>
          )}
        </CardHeader>
        <CardContent>
          {activeTab === "members" && (
            <DataTable
              columns={memberColumns as ColumnDef<TeamMember>[]}
              data={members?.data || []}
              isLoading={membersLoading}
              emptyMessage="No members in this team."
              searchColumn="name"
              searchPlaceholder="Search members..."
            />
          )}
          {activeTab === "usage" && orgSlug && teamSlug && (
            <UsageDashboard scope={{ type: "team", orgSlug, teamSlug }} />
          )}
        </CardContent>
      </Card>

      {/* Edit Modal */}
      <Modal open={isEditModalOpen} onClose={() => setIsEditModalOpen(false)}>
        <form onSubmit={editForm.handleSubmit(onEditSubmit)}>
          <ModalHeader>Edit Team</ModalHeader>
          <ModalContent>
            <FormField
              label="Name"
              htmlFor="name"
              required
              error={editForm.formState.errors.name?.message}
            >
              <Input id="name" {...editForm.register("name")} placeholder="Team Name" />
            </FormField>
          </ModalContent>
          <ModalFooter>
            <Button type="button" variant="ghost" onClick={() => setIsEditModalOpen(false)}>
              Cancel
            </Button>
            <Button type="submit" isLoading={updateMutation.isPending}>
              Save
            </Button>
          </ModalFooter>
        </form>
      </Modal>

      {/* Add Member Modal */}
      <AddMemberModal
        open={isAddMemberModalOpen}
        onClose={() => setIsAddMemberModalOpen(false)}
        onSubmit={handleAddMember}
        availableUsers={availableUsers || []}
        isLoading={addMemberMutation.isPending}
        emptyMessage="All users are already members of this team."
      />
    </div>
  );
}
