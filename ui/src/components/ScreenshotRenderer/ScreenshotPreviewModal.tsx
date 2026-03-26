import { useCallback, useEffect, useRef, useState } from "react";
import { Check, Copy, Download } from "lucide-react";

import { Button } from "@/components/Button/Button";
import {
  Modal,
  ModalClose,
  ModalContent,
  ModalFooter,
  ModalHeader,
  ModalTitle,
} from "@/components/Modal/Modal";
import { useToast } from "@/components/Toast/Toast";
import { downloadBlob, generateScreenshotFilename } from "@/utils/exportScreenshot";

interface ScreenshotPreviewModalProps {
  open: boolean;
  onClose: () => void;
  imageUrl: string;
  blob: Blob;
  title: string;
}

export function ScreenshotPreviewModal({
  open,
  onClose,
  imageUrl,
  blob,
  title,
}: ScreenshotPreviewModalProps) {
  const [copied, setCopied] = useState(false);
  const toast = useToast();
  const copyTimerRef = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    return () => clearTimeout(copyTimerRef.current);
  }, []);

  const handleDownload = useCallback(() => {
    downloadBlob(blob, generateScreenshotFilename(title));
  }, [blob, title]);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.write([new ClipboardItem({ [blob.type]: blob })]);
      setCopied(true);
      clearTimeout(copyTimerRef.current);
      copyTimerRef.current = setTimeout(() => setCopied(false), 2000);
    } catch {
      toast.error("Copy failed", "Your browser may not support copying images");
    }
  }, [blob, toast]);

  return (
    <Modal open={open} onClose={onClose} className="max-w-5xl w-[92vw] max-h-[90vh] flex flex-col">
      <ModalClose onClose={onClose} />
      <ModalHeader>
        <ModalTitle>Screenshot Preview</ModalTitle>
      </ModalHeader>

      <ModalContent className="flex-1 overflow-auto -mx-6 px-0">
        <img src={imageUrl} alt={`Screenshot of ${title}`} className="w-full" />
      </ModalContent>

      <ModalFooter>
        <Button variant="ghost" onClick={handleCopy} className="gap-2">
          {copied ? <Check className="h-4 w-4 text-success" /> : <Copy className="h-4 w-4" />}
          {copied ? "Copied" : "Copy to clipboard"}
        </Button>
        <Button onClick={handleDownload} className="gap-2">
          <Download className="h-4 w-4" />
          Download
        </Button>
      </ModalFooter>
    </Modal>
  );
}
