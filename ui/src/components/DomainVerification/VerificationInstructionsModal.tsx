import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Copy, Check, RefreshCw, CheckCircle2, XCircle, Clock } from "lucide-react";
import { useState } from "react";

import {
  domainVerificationsGetOptions,
  domainVerificationsGetInstructionsOptions,
  domainVerificationsVerifyMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import type { DomainVerificationStatus } from "@/api/generated/types.gen";
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
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { Badge, type BadgeVariant } from "@/components/Badge/Badge";
import { useToast } from "@/components/Toast/Toast";

const statusVariantMap: Record<DomainVerificationStatus, BadgeVariant> = {
  pending: "warning",
  verified: "success",
  failed: "destructive",
};

const statusIconMap: Record<DomainVerificationStatus, React.ReactNode> = {
  pending: <Clock className="h-4 w-4" />,
  verified: <CheckCircle2 className="h-4 w-4" />,
  failed: <XCircle className="h-4 w-4" />,
};

export interface VerificationInstructionsModalProps {
  open: boolean;
  onClose: () => void;
  orgSlug: string;
  domainId: string;
}

export function VerificationInstructionsModal({
  open,
  onClose,
  orgSlug,
  domainId,
}: VerificationInstructionsModalProps) {
  const { toast } = useToast();
  const queryClient = useQueryClient();
  const [copiedField, setCopiedField] = useState<"host" | "value" | null>(null);

  const { data: domain, isLoading: domainLoading } = useQuery({
    ...domainVerificationsGetOptions({ path: { org_slug: orgSlug, domain_id: domainId } }),
    enabled: open && !!domainId,
  });

  const { data: instructions, isLoading: instructionsLoading } = useQuery({
    ...domainVerificationsGetInstructionsOptions({
      path: { org_slug: orgSlug, domain_id: domainId },
    }),
    enabled: open && !!domainId,
  });

  const verifyMutation = useMutation({
    ...domainVerificationsVerifyMutation(),
    onSuccess: (response) => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "domainVerificationsList" }] });
      queryClient.invalidateQueries({ queryKey: [{ _id: "domainVerificationsGet" }] });
      if (response.verified) {
        toast({ title: "Domain verified successfully", type: "success" });
      } else {
        toast({
          title: "Verification failed",
          description: response.message,
          type: "warning",
        });
      }
    },
    onError: (error) => {
      toast({
        title: "Verification failed",
        description: String(error),
        type: "error",
      });
    },
  });

  const handleCopy = async (value: string, field: "host" | "value") => {
    try {
      await navigator.clipboard.writeText(value);
      setCopiedField(field);
      setTimeout(() => setCopiedField(null), 2000);
    } catch {
      toast({
        title: "Failed to copy",
        description: "Please copy the value manually",
        type: "error",
      });
    }
  };

  const handleVerify = () => {
    verifyMutation.mutate({
      path: { org_slug: orgSlug, domain_id: domainId },
    });
  };

  const isLoading = domainLoading || instructionsLoading;
  const isVerified = domain?.status === "verified";

  return (
    <Modal open={open} onClose={onClose}>
      <ModalClose onClose={onClose} />
      <ModalHeader>
        <ModalTitle>DNS Verification Instructions</ModalTitle>
        {domain && (
          <ModalDescription>
            Follow these steps to verify ownership of <strong>{domain.domain}</strong>
          </ModalDescription>
        )}
      </ModalHeader>
      <ModalContent>
        {isLoading ? (
          <div className="space-y-4">
            <Skeleton className="h-4 w-48" />
            <Skeleton className="h-20 w-full" />
            <Skeleton className="h-20 w-full" />
          </div>
        ) : domain && instructions ? (
          <div className="space-y-6">
            {/* Status */}
            <div className="flex items-center gap-2">
              <span className="text-sm text-muted-foreground">Status:</span>
              <Badge variant={statusVariantMap[domain.status]} className="flex items-center gap-1">
                {statusIconMap[domain.status]}
                {domain.status.charAt(0).toUpperCase() + domain.status.slice(1)}
              </Badge>
            </div>

            {!isVerified && (
              <>
                {/* Instructions */}
                <div className="space-y-2">
                  <p className="text-sm text-muted-foreground">
                    Add the following DNS TXT record to verify domain ownership:
                  </p>
                </div>

                {/* DNS Record Host */}
                <div className="space-y-2">
                  <span className="text-sm font-medium">Record Host / Name</span>
                  <div className="flex items-center gap-2">
                    <code className="flex-1 rounded bg-muted px-3 py-2 text-sm font-mono break-all">
                      {instructions.record_host}
                    </code>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => handleCopy(instructions.record_host, "host")}
                      aria-label="Copy DNS record host"
                    >
                      {copiedField === "host" ? (
                        <Check className="h-4 w-4 text-success" />
                      ) : (
                        <Copy className="h-4 w-4" />
                      )}
                    </Button>
                  </div>
                </div>

                {/* DNS Record Type */}
                <div className="space-y-2">
                  <span className="text-sm font-medium">Record Type</span>
                  <code className="block rounded bg-muted px-3 py-2 text-sm font-mono">
                    {instructions.record_type}
                  </code>
                </div>

                {/* DNS Record Value */}
                <div className="space-y-2">
                  <span className="text-sm font-medium">Record Value</span>
                  <div className="flex items-center gap-2">
                    <code className="flex-1 rounded bg-muted px-3 py-2 text-sm font-mono break-all">
                      {instructions.record_value}
                    </code>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => handleCopy(instructions.record_value, "value")}
                      aria-label="Copy DNS record value"
                    >
                      {copiedField === "value" ? (
                        <Check className="h-4 w-4 text-success" />
                      ) : (
                        <Copy className="h-4 w-4" />
                      )}
                    </Button>
                  </div>
                </div>

                {/* Note */}
                <div className="rounded-lg border bg-muted/30 p-4 text-sm text-muted-foreground">
                  <p>
                    DNS changes can take up to 72 hours to propagate. Click "Verify Now" once you've
                    added the record.
                  </p>
                </div>
              </>
            )}

            {isVerified && (
              <div className="rounded-lg border border-success/20 bg-success/10 p-4">
                <div className="flex items-center gap-2 text-success">
                  <CheckCircle2 className="h-5 w-5" />
                  <p className="font-medium">Domain verified</p>
                </div>
                <p className="mt-1 text-sm text-muted-foreground">
                  This domain has been verified and can be used for SSO enforcement.
                </p>
              </div>
            )}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground">Failed to load instructions</p>
        )}
      </ModalContent>
      <ModalFooter>
        <Button variant="secondary" onClick={onClose}>
          Close
        </Button>
        {!isVerified && (
          <Button onClick={handleVerify} disabled={verifyMutation.isPending || isLoading}>
            {verifyMutation.isPending ? (
              <>
                <RefreshCw className="mr-2 h-4 w-4 animate-spin" />
                Verifying...
              </>
            ) : (
              <>
                <RefreshCw className="mr-2 h-4 w-4" />
                Verify Now
              </>
            )}
          </Button>
        )}
      </ModalFooter>
    </Modal>
  );
}
