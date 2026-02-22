import { useState } from "react";
import { AlertCircle, Copy, Check } from "lucide-react";

import { Button } from "@/components/Button/Button";
import { Modal, ModalHeader, ModalContent, ModalFooter } from "@/components/Modal/Modal";

export interface ApiKeyCreatedModalProps {
  apiKey: string | null;
  onClose: () => void;
}

export function ApiKeyCreatedModal({ apiKey, onClose }: ApiKeyCreatedModalProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    if (apiKey) {
      await navigator.clipboard.writeText(apiKey);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const handleClose = () => {
    setCopied(false);
    onClose();
  };

  return (
    <Modal open={!!apiKey} onClose={handleClose}>
      <ModalHeader>API Key Created</ModalHeader>
      <ModalContent>
        <div className="space-y-4">
          <div className="flex items-center gap-2 rounded-md border border-warning bg-warning/10 p-3 text-warning">
            <AlertCircle className="h-5 w-5 shrink-0" />
            <p className="text-sm">
              Make sure to copy your API key now. You won't be able to see it again!
            </p>
          </div>
          <div className="flex items-center gap-2">
            <code className="flex-1 rounded-md bg-muted p-3 text-sm break-all">{apiKey}</code>
            <Button
              variant="outline"
              size="sm"
              onClick={handleCopy}
              aria-label={copied ? "Copied" : "Copy API key"}
            >
              {copied ? <Check className="h-4 w-4 text-success" /> : <Copy className="h-4 w-4" />}
            </Button>
          </div>
        </div>
      </ModalContent>
      <ModalFooter>
        <Button onClick={handleClose}>Done</Button>
      </ModalFooter>
    </Modal>
  );
}
