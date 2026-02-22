import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Eye, RefreshCw, Trash2, Globe, Plus } from "lucide-react";
import { useState } from "react";

import {
  domainVerificationsListOptions,
  domainVerificationsDeleteMutation,
  domainVerificationsVerifyMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import type { DomainVerification, DomainVerificationStatus } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { Badge, type BadgeVariant } from "@/components/Badge/Badge";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { useToast } from "@/components/Toast/Toast";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";
import { formatDateTime } from "@/utils/formatters";

const statusVariantMap: Record<DomainVerificationStatus, BadgeVariant> = {
  pending: "warning",
  verified: "success",
  failed: "destructive",
};

const statusLabelMap: Record<DomainVerificationStatus, string> = {
  pending: "Pending",
  verified: "Verified",
  failed: "Failed",
};

export interface DomainVerificationListProps {
  orgSlug: string;
  onAddDomain: () => void;
  onViewInstructions: (domainId: string) => void;
}

export function DomainVerificationList({
  orgSlug,
  onAddDomain,
  onViewInstructions,
}: DomainVerificationListProps) {
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();

  const [verifyingDomainId, setVerifyingDomainId] = useState<string | null>(null);

  const { data, isLoading, error } = useQuery({
    ...domainVerificationsListOptions({ path: { org_slug: orgSlug } }),
  });

  const deleteMutation = useMutation({
    ...domainVerificationsDeleteMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "domainVerificationsList" }] });
      toast({ title: "Domain removed", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to remove domain",
        description: String(error),
        type: "error",
      });
    },
  });

  const verifyMutation = useMutation({
    ...domainVerificationsVerifyMutation(),
    onSuccess: (response) => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "domainVerificationsList" }] });
      setVerifyingDomainId(null);
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
      setVerifyingDomainId(null);
      toast({
        title: "Verification failed",
        description: String(error),
        type: "error",
      });
    },
  });

  const handleDelete = async (domain: DomainVerification) => {
    const confirmed = await confirm({
      title: "Remove Domain",
      message: `Are you sure you want to remove "${domain.domain}"? This will also remove any verification status.`,
      confirmLabel: "Remove",
      variant: "destructive",
    });
    if (confirmed) {
      deleteMutation.mutate({
        path: { org_slug: orgSlug, domain_id: domain.id },
      });
    }
  };

  const handleVerify = (domain: DomainVerification) => {
    setVerifyingDomainId(domain.id);
    verifyMutation.mutate({
      path: { org_slug: orgSlug, domain_id: domain.id },
    });
  };

  if (isLoading) {
    return (
      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle className="text-base">Domain Verification</CardTitle>
          <Skeleton className="h-9 w-28" />
        </CardHeader>
        <CardContent>
          <div className="space-y-3">
            <Skeleton className="h-12 w-full" />
            <Skeleton className="h-12 w-full" />
          </div>
        </CardContent>
      </Card>
    );
  }

  if (error) {
    return (
      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle className="text-base">Domain Verification</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-destructive">Failed to load domains</p>
        </CardContent>
      </Card>
    );
  }

  const domains = data?.items ?? [];

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between">
        <CardTitle className="text-base">Domain Verification</CardTitle>
        <Button size="sm" onClick={onAddDomain}>
          <Plus className="mr-2 h-4 w-4" />
          Add Domain
        </Button>
      </CardHeader>
      <CardContent>
        {domains.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-8 text-center">
            <div className="flex h-12 w-12 items-center justify-center rounded-full bg-muted">
              <Globe className="h-6 w-6 text-muted-foreground" />
            </div>
            <h3 className="mt-4 text-sm font-medium">No domains verified</h3>
            <p className="mt-1 max-w-sm text-sm text-muted-foreground">
              Add and verify domains to prove ownership before enforcing SSO for users with those
              email domains.
            </p>
          </div>
        ) : (
          <div className="divide-y">
            {domains.map((domain) => (
              <div
                key={domain.id}
                className="flex items-center justify-between py-3 first:pt-0 last:pb-0"
              >
                <div className="flex items-center gap-3">
                  <Globe className="h-4 w-4 text-muted-foreground" />
                  <div>
                    <p className="font-medium">{domain.domain}</p>
                    <div className="flex items-center gap-2 text-xs text-muted-foreground">
                      <span>
                        {domain.verification_attempts}{" "}
                        {domain.verification_attempts === 1 ? "attempt" : "attempts"}
                      </span>
                      {domain.verified_at && (
                        <>
                          <span>·</span>
                          <span>Verified {formatDateTime(domain.verified_at)}</span>
                        </>
                      )}
                    </div>
                  </div>
                </div>
                <div className="flex items-center ml-4 gap-2">
                  <Badge variant={statusVariantMap[domain.status]}>
                    {statusLabelMap[domain.status]}
                  </Badge>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => onViewInstructions(domain.id)}
                    aria-label="View DNS verification instructions"
                  >
                    <Eye className="h-4 w-4" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleVerify(domain)}
                    disabled={verifyingDomainId === domain.id || domain.status === "verified"}
                    aria-label="Verify domain ownership"
                  >
                    <RefreshCw
                      className={`h-4 w-4 ${verifyingDomainId === domain.id ? "animate-spin" : ""}`}
                    />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleDelete(domain)}
                    disabled={deleteMutation.isPending}
                    aria-label="Remove domain"
                  >
                    <Trash2 className="h-4 w-4 text-destructive" />
                  </Button>
                </div>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
