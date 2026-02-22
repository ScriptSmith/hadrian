import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";

import { domainVerificationsCreateMutation } from "@/api/generated/@tanstack/react-query.gen";
import { Button } from "@/components/Button/Button";
import {
  Modal,
  ModalClose,
  ModalContent,
  ModalFooter,
  ModalHeader,
  ModalTitle,
  ModalDescription,
} from "@/components/Modal/Modal";
import { Input } from "@/components/Input/Input";
import { Label } from "@/components/Label/Label";
import { useToast } from "@/components/Toast/Toast";

const domainSchema = z.object({
  domain: z
    .string()
    .min(1, "Domain is required")
    .regex(
      /^(?:[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]{2,}$/,
      "Please enter a valid domain (e.g., acme.com)"
    ),
});

type DomainFormValues = z.infer<typeof domainSchema>;

export interface AddDomainModalProps {
  open: boolean;
  onClose: () => void;
  orgSlug: string;
}

export function AddDomainModal({ open, onClose, orgSlug }: AddDomainModalProps) {
  const { toast } = useToast();
  const queryClient = useQueryClient();

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors },
  } = useForm<DomainFormValues>({
    resolver: zodResolver(domainSchema),
    defaultValues: { domain: "" },
  });

  const createMutation = useMutation({
    ...domainVerificationsCreateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "domainVerificationsList" }] });
      toast({
        title: "Domain added",
        description: "Follow the DNS instructions to verify ownership.",
        type: "success",
      });
      reset();
      onClose();
    },
    onError: (error) => {
      toast({
        title: "Failed to add domain",
        description: String(error),
        type: "error",
      });
    },
  });

  const onSubmit = (data: DomainFormValues) => {
    createMutation.mutate({
      path: { org_slug: orgSlug },
      body: { domain: data.domain.toLowerCase().trim() },
    });
  };

  const handleClose = () => {
    reset();
    onClose();
  };

  return (
    <Modal open={open} onClose={handleClose}>
      <ModalClose onClose={handleClose} />
      <ModalHeader>
        <ModalTitle>Add Domain</ModalTitle>
        <ModalDescription>
          Add a domain to verify ownership via DNS. You'll need to add a TXT record to your DNS
          configuration.
        </ModalDescription>
      </ModalHeader>
      <form onSubmit={handleSubmit(onSubmit)}>
        <ModalContent>
          <div className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="domain">Domain</Label>
              <Input
                id="domain"
                placeholder="acme.com"
                {...register("domain")}
                aria-invalid={!!errors.domain}
              />
              {errors.domain && <p className="text-sm text-destructive">{errors.domain.message}</p>}
              <p className="text-xs text-muted-foreground">
                Enter the email domain you want to verify (e.g., acme.com for @acme.com email
                addresses).
              </p>
            </div>
          </div>
        </ModalContent>
        <ModalFooter>
          <Button type="button" variant="secondary" onClick={handleClose}>
            Cancel
          </Button>
          <Button type="submit" disabled={createMutation.isPending}>
            {createMutation.isPending ? "Adding..." : "Add Domain"}
          </Button>
        </ModalFooter>
      </form>
    </Modal>
  );
}
