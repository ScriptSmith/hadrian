import { zodResolver } from "@hookform/resolvers/zod";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { createColumnHelper } from "@tanstack/react-table";
import { MoreHorizontal, Pencil, Mail } from "lucide-react";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { Link } from "react-router-dom";
import { z } from "zod";

import {
  userListOptions,
  userCreateMutation,
  userUpdateMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import type { User } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import {
  Dropdown,
  DropdownContent,
  DropdownItem,
  DropdownTrigger,
} from "@/components/Dropdown/Dropdown";
import { FormField } from "@/components/FormField/FormField";
import { Input } from "@/components/Input/Input";
import { Modal, ModalHeader, ModalContent, ModalFooter } from "@/components/Modal/Modal";
import { useToast } from "@/components/Toast/Toast";
import { PageHeader, ResourceTable } from "@/components/Admin";
import { useCursorPagination } from "@/hooks";
import { formatDateTime } from "@/utils/formatters";

const columnHelper = createColumnHelper<User>();

const createUserSchema = z.object({
  external_id: z.string().min(1, "External ID is required"),
  name: z.string().optional(),
  email: z.string().email("Invalid email address").optional().or(z.literal("")),
});

type CreateUserForm = z.infer<typeof createUserSchema>;

const editUserSchema = z.object({
  name: z.string().optional(),
  email: z.string().email("Invalid email address").optional().or(z.literal("")),
});

type EditUserForm = z.infer<typeof editUserSchema>;

export default function UsersPage() {
  const { toast } = useToast();
  const queryClient = useQueryClient();
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [isEditModalOpen, setIsEditModalOpen] = useState(false);
  const [editingUser, setEditingUser] = useState<User | null>(null);

  const createForm = useForm<CreateUserForm>({
    resolver: zodResolver(createUserSchema),
    defaultValues: {
      external_id: "",
      name: "",
      email: "",
    },
  });

  const editForm = useForm<EditUserForm>({
    resolver: zodResolver(editUserSchema),
    defaultValues: {
      name: "",
      email: "",
    },
  });

  const pagination = useCursorPagination({ defaultLimit: 25 });

  const {
    data: users,
    isLoading,
    error,
  } = useQuery(userListOptions({ query: pagination.queryParams }));

  const createMutation = useMutation({
    ...userCreateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "userList" }] });
      setIsCreateModalOpen(false);
      createForm.reset();
      toast({ title: "User created", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to create user", description: String(error), type: "error" });
    },
  });

  const updateMutation = useMutation({
    ...userUpdateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "userList" }] });
      setIsEditModalOpen(false);
      setEditingUser(null);
      toast({ title: "User updated", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to update user", description: String(error), type: "error" });
    },
  });

  const handleEdit = (user: User) => {
    setEditingUser(user);
    editForm.reset({ name: user.name ?? "", email: user.email ?? "" });
    setIsEditModalOpen(true);
  };

  const columns = [
    columnHelper.accessor("name", {
      header: "Name",
      cell: (info) => (
        <Link
          to={`/admin/users/${info.row.original.id}`}
          className="font-medium text-primary hover:underline"
        >
          {info.getValue() || info.row.original.email || info.row.original.external_id}
        </Link>
      ),
    }),
    columnHelper.accessor("email", {
      header: "Email",
      cell: (info) => {
        const email = info.getValue();
        return email ? (
          <a
            href={`mailto:${email}`}
            className="flex items-center gap-1 text-primary hover:underline"
          >
            <Mail className="h-3 w-3" />
            {email}
          </a>
        ) : (
          <span className="text-muted-foreground">—</span>
        );
      },
    }),
    columnHelper.accessor("external_id", {
      header: "External ID",
      cell: (info) => <CodeBadge>{info.getValue()}</CodeBadge>,
    }),
    columnHelper.accessor("created_at", {
      header: "Created",
      cell: (info) => formatDateTime(info.getValue()),
    }),
    columnHelper.display({
      id: "actions",
      cell: ({ row }) => (
        <Dropdown>
          <DropdownTrigger aria-label="Actions" variant="ghost" className="h-8 w-8 p-0">
            <MoreHorizontal className="h-4.5 w-4.5" />
          </DropdownTrigger>
          <DropdownContent align="end">
            <DropdownItem onClick={() => handleEdit(row.original)}>
              <Pencil className="mr-2 h-4 w-4" />
              Edit
            </DropdownItem>
          </DropdownContent>
        </Dropdown>
      ),
    }),
  ];

  const onCreateSubmit = (data: CreateUserForm) => {
    createMutation.mutate({
      body: {
        external_id: data.external_id,
        name: data.name || null,
        email: data.email || null,
      },
    });
  };

  const onEditSubmit = (data: EditUserForm) => {
    if (!editingUser) return;
    updateMutation.mutate({
      path: { user_id: editingUser.id },
      body: {
        name: data.name || null,
        email: data.email || null,
      },
    });
  };

  return (
    <div className="p-6">
      <PageHeader
        title="Users"
        description="Manage users and their permissions"
        actionLabel="New User"
        onAction={() => setIsCreateModalOpen(true)}
      />

      <ResourceTable
        title="All Users"
        columns={columns}
        data={users?.data || []}
        isLoading={isLoading}
        error={error}
        emptyMessage="No users yet. Create one to get started."
        errorMessage="Failed to load users. Please try again."
        paginationProps={{
          pagination: users?.pagination,
          isFirstPage: pagination.info.isFirstPage,
          pageNumber: pagination.info.pageNumber,
          onPrevious: () => pagination.actions.goToPreviousPage(users!.pagination),
          onNext: () => pagination.actions.goToNextPage(users!.pagination),
          onFirst: () => pagination.actions.goToFirstPage(),
        }}
      />

      {/* Create User Modal */}
      <Modal open={isCreateModalOpen} onClose={() => setIsCreateModalOpen(false)}>
        <form onSubmit={createForm.handleSubmit(onCreateSubmit)}>
          <ModalHeader>Create User</ModalHeader>
          <ModalContent>
            <div className="space-y-4">
              <FormField
                label="External ID"
                htmlFor="external_id"
                required
                helpText="Unique identifier from your SSO provider"
                error={createForm.formState.errors.external_id?.message}
              >
                <Input
                  id="external_id"
                  {...createForm.register("external_id")}
                  placeholder="user_123"
                />
              </FormField>
              <FormField
                label="Name"
                htmlFor="name"
                error={createForm.formState.errors.name?.message}
              >
                <Input id="name" {...createForm.register("name")} placeholder="John Doe" />
              </FormField>
              <FormField
                label="Email"
                htmlFor="email"
                error={createForm.formState.errors.email?.message}
              >
                <Input
                  id="email"
                  type="email"
                  {...createForm.register("email")}
                  placeholder="john@example.com"
                />
              </FormField>
            </div>
          </ModalContent>
          <ModalFooter>
            <Button type="button" variant="ghost" onClick={() => setIsCreateModalOpen(false)}>
              Cancel
            </Button>
            <Button type="submit" isLoading={createMutation.isPending}>
              Create
            </Button>
          </ModalFooter>
        </form>
      </Modal>

      {/* Edit User Modal */}
      <Modal open={isEditModalOpen} onClose={() => setIsEditModalOpen(false)}>
        <form onSubmit={editForm.handleSubmit(onEditSubmit)}>
          <ModalHeader>Edit User</ModalHeader>
          <ModalContent>
            <div className="space-y-4">
              <FormField
                label="Name"
                htmlFor="edit-name"
                error={editForm.formState.errors.name?.message}
              >
                <Input id="edit-name" {...editForm.register("name")} placeholder="John Doe" />
              </FormField>
              <FormField
                label="Email"
                htmlFor="edit-email"
                error={editForm.formState.errors.email?.message}
              >
                <Input
                  id="edit-email"
                  type="email"
                  {...editForm.register("email")}
                  placeholder="john@example.com"
                />
              </FormField>
            </div>
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
    </div>
  );
}
