import { zodResolver } from "@hookform/resolvers/zod";
import { useEffect } from "react";
import { useForm, Controller } from "react-hook-form";
import { z } from "zod";

import type { User } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { FormField } from "@/components/FormField/FormField";
import { Modal, ModalHeader, ModalContent, ModalFooter } from "@/components/Modal/Modal";
import { Select } from "@/components/Select/Select";

const addMemberSchema = z.object({
  userId: z.string().min(1, "Please select a user"),
});

type AddMemberForm = z.infer<typeof addMemberSchema>;

export interface AddMemberModalProps {
  open: boolean;
  onClose: () => void;
  onSubmit: (userId: string) => void;
  availableUsers: User[];
  isLoading?: boolean;
  title?: string;
  emptyMessage?: string;
}

export function AddMemberModal({
  open,
  onClose,
  onSubmit,
  availableUsers,
  isLoading = false,
  title = "Add Member",
  emptyMessage = "All users are already members.",
}: AddMemberModalProps) {
  const form = useForm<AddMemberForm>({
    resolver: zodResolver(addMemberSchema),
    defaultValues: { userId: "" },
  });

  // Reset form when modal closes
  useEffect(() => {
    if (!open) {
      form.reset();
    }
  }, [open, form]);

  const handleFormSubmit = (data: AddMemberForm) => {
    onSubmit(data.userId);
  };

  return (
    <Modal open={open} onClose={onClose}>
      <form onSubmit={form.handleSubmit(handleFormSubmit)}>
        <ModalHeader>{title}</ModalHeader>
        <ModalContent>
          <div className="space-y-4">
            <FormField
              label="Select User"
              htmlFor="user"
              error={form.formState.errors.userId?.message}
            >
              <Controller
                name="userId"
                control={form.control}
                render={({ field }) => (
                  <Select
                    value={field.value || null}
                    onChange={(value) => field.onChange(value || "")}
                    placeholder="Select a user..."
                    searchable
                    options={availableUsers.map((user) => ({
                      value: user.id,
                      label: user.name || user.email || user.external_id,
                    }))}
                  />
                )}
              />
              {availableUsers.length === 0 && (
                <p className="mt-2 text-sm text-muted-foreground">{emptyMessage}</p>
              )}
            </FormField>
          </div>
        </ModalContent>
        <ModalFooter>
          <Button type="button" variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" isLoading={isLoading} disabled={!form.watch("userId")}>
            Add Member
          </Button>
        </ModalFooter>
      </form>
    </Modal>
  );
}
