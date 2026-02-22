import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { History, RotateCcw, ChevronDown, ChevronRight } from "lucide-react";

import {
  orgRbacPolicyListVersionsOptions,
  orgRbacPolicyRollbackMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import type { OrgRbacPolicyVersion, OrgRbacPolicy } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Badge } from "@/components/Badge/Badge";
import {
  Modal,
  ModalHeader,
  ModalTitle,
  ModalContent,
  ModalFooter,
} from "@/components/Modal/Modal";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { useToast } from "@/components/Toast/Toast";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";
import { formatDateTime } from "@/utils/formatters";

interface RbacPolicyVersionHistoryModalProps {
  open: boolean;
  onClose: () => void;
  policy: OrgRbacPolicy;
  orgSlug: string;
}

function VersionItem({
  version,
  isLatest,
  onRollback,
  isRollingBack,
}: {
  version: OrgRbacPolicyVersion;
  isLatest: boolean;
  onRollback: () => void;
  isRollingBack: boolean;
}) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="border rounded-lg">
      <div className="flex items-center justify-between p-3">
        <button
          type="button"
          className="flex flex-1 items-center gap-3 text-left rounded hover:bg-muted/50 transition-colors p-1 -m-1"
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? (
            <ChevronDown className="h-4 w-4 text-muted-foreground" />
          ) : (
            <ChevronRight className="h-4 w-4 text-muted-foreground" />
          )}
          <div>
            <div className="flex items-center gap-2">
              <span className="font-medium">Version {version.version}</span>
              {isLatest && <Badge variant="secondary">Current</Badge>}
              <Badge variant={version.effect === "allow" ? "default" : "destructive"}>
                {version.effect}
              </Badge>
              <Badge variant={version.enabled ? "default" : "secondary"}>
                {version.enabled ? "Enabled" : "Disabled"}
              </Badge>
            </div>
            <p className="text-xs text-muted-foreground mt-0.5">
              {formatDateTime(version.created_at)}
              {version.created_by && ` by ${version.created_by}`}
            </p>
          </div>
        </button>
        {!isLatest && (
          <Button
            variant="ghost"
            size="sm"
            onClick={onRollback}
            disabled={isRollingBack}
            isLoading={isRollingBack}
          >
            <RotateCcw className="h-4 w-4 mr-1" />
            Rollback
          </Button>
        )}
      </div>

      {expanded && (
        <div className="border-t p-3 space-y-3 bg-muted/30">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <p className="text-xs font-medium text-muted-foreground">Name</p>
              <p className="text-sm">{version.name}</p>
            </div>
            <div>
              <p className="text-xs font-medium text-muted-foreground">Priority</p>
              <p className="text-sm">{version.priority}</p>
            </div>
          </div>

          {version.description && (
            <div>
              <p className="text-xs font-medium text-muted-foreground">Description</p>
              <p className="text-sm">{version.description}</p>
            </div>
          )}

          <div className="grid grid-cols-2 gap-4">
            <div>
              <p className="text-xs font-medium text-muted-foreground">Resource</p>
              <code className="text-xs bg-muted px-1 py-0.5 rounded">{version.resource}</code>
            </div>
            <div>
              <p className="text-xs font-medium text-muted-foreground">Action</p>
              <code className="text-xs bg-muted px-1 py-0.5 rounded">{version.action}</code>
            </div>
          </div>

          <div>
            <p className="text-xs font-medium text-muted-foreground">Condition</p>
            <pre className="text-xs bg-muted p-2 rounded mt-1 overflow-x-auto font-mono">
              {version.condition}
            </pre>
          </div>

          {version.reason && (
            <div>
              <p className="text-xs font-medium text-muted-foreground">Change Reason</p>
              <p className="text-sm italic text-muted-foreground">{version.reason}</p>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export function RbacPolicyVersionHistoryModal({
  open,
  onClose,
  policy,
  orgSlug,
}: RbacPolicyVersionHistoryModalProps) {
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const [rollbackVersion, setRollbackVersion] = useState<number | null>(null);

  const { data, isLoading } = useQuery({
    ...orgRbacPolicyListVersionsOptions({
      path: { org_slug: orgSlug, policy_id: policy.id },
    }),
    enabled: open,
  });

  const rollbackMutation = useMutation({
    ...orgRbacPolicyRollbackMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "orgRbacPolicyList" }] });
      queryClient.invalidateQueries({ queryKey: [{ _id: "orgRbacPolicyListVersions" }] });
      toast({ title: "Policy rolled back successfully", type: "success" });
      setRollbackVersion(null);
      onClose();
    },
    onError: (error) => {
      toast({
        title: "Failed to rollback policy",
        description: String(error),
        type: "error",
      });
      setRollbackVersion(null);
    },
  });

  const handleRollback = async (version: OrgRbacPolicyVersion) => {
    const confirmed = await confirm({
      title: "Rollback Policy",
      message: `Are you sure you want to rollback to version ${version.version}? This will create a new version with the previous settings.`,
      confirmLabel: "Rollback",
      variant: "default",
    });

    if (confirmed) {
      setRollbackVersion(version.version);
      rollbackMutation.mutate({
        path: { org_slug: orgSlug, policy_id: policy.id },
        body: {
          target_version: version.version,
          reason: `Rolled back to version ${version.version}`,
        },
      });
    }
  };

  const versions = data?.data ?? [];

  return (
    <Modal open={open} onClose={onClose}>
      <ModalHeader>
        <ModalTitle className="flex items-center gap-2">
          <History className="h-5 w-5" />
          Version History
        </ModalTitle>
      </ModalHeader>
      <ModalContent className="max-h-[60vh] overflow-y-auto">
        <div className="space-y-2">
          <p className="text-sm text-muted-foreground mb-4">
            Policy: <span className="font-medium text-foreground">{policy.name}</span>
          </p>

          {isLoading ? (
            <div className="space-y-3">
              {[1, 2, 3].map((i) => (
                <Skeleton key={i} className="h-16 w-full" />
              ))}
            </div>
          ) : versions.length === 0 ? (
            <p className="text-center py-8 text-muted-foreground">No version history available</p>
          ) : (
            <div className="space-y-2">
              {versions.map((version, index) => (
                <VersionItem
                  key={version.id}
                  version={version}
                  isLatest={index === 0}
                  onRollback={() => handleRollback(version)}
                  isRollingBack={rollbackVersion === version.version}
                />
              ))}
            </div>
          )}
        </div>
      </ModalContent>
      <ModalFooter>
        <Button variant="ghost" onClick={onClose}>
          Close
        </Button>
      </ModalFooter>
    </Modal>
  );
}
