import { useState } from "react";
import { AlertCircle, Copy, Check, Info } from "lucide-react";

import { Button } from "@/components/Button/Button";
import { Modal, ModalHeader, ModalContent, ModalFooter } from "@/components/Modal/Modal";

export interface ScimTokenCreatedModalProps {
  token: string | null;
  onClose: () => void;
}

export function ScimTokenCreatedModal({ token, onClose }: ScimTokenCreatedModalProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    if (token) {
      await navigator.clipboard.writeText(token);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const handleClose = () => {
    setCopied(false);
    onClose();
  };

  return (
    <Modal open={!!token} onClose={handleClose}>
      <ModalHeader>SCIM Token Created</ModalHeader>
      <ModalContent>
        <div className="space-y-4">
          <div className="flex items-center gap-2 rounded-md border border-warning bg-warning/10 p-3 text-warning">
            <AlertCircle className="h-5 w-5 shrink-0" />
            <p className="text-sm">
              Make sure to copy your SCIM bearer token now. You won't be able to see it again!
            </p>
          </div>
          <div className="flex items-center gap-2">
            <code className="flex-1 rounded-md bg-muted p-3 text-sm break-all">{token}</code>
            <Button variant="outline" size="sm" onClick={handleCopy} aria-label="Copy token">
              {copied ? <Check className="h-4 w-4 text-success" /> : <Copy className="h-4 w-4" />}
            </Button>
          </div>
          <div className="flex items-start gap-2 rounded-md border bg-muted/30 p-3">
            <Info className="h-5 w-5 text-muted-foreground mt-0.5 shrink-0" />
            <p className="text-sm text-muted-foreground">
              Use this token in your identity provider's SCIM configuration as the Authorization
              bearer token. The SCIM endpoint is{" "}
              <code className="text-xs bg-muted px-1 py-0.5 rounded">/scim/v2/</code>
            </p>
          </div>
        </div>
      </ModalContent>
      <ModalFooter>
        <Button onClick={handleClose}>Done</Button>
      </ModalFooter>
    </Modal>
  );
}
