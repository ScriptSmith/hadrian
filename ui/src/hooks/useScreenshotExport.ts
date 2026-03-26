import { useCallback, useRef, useState } from "react";

import { useToast } from "@/components/Toast/Toast";

export interface ScreenshotResult {
  blob: Blob;
  url: string;
}

export function useScreenshotExport() {
  const [isCapturing, setIsCapturing] = useState(false);
  const [screenshot, setScreenshot] = useState<ScreenshotResult | null>(null);
  const objectUrlRef = useRef<string | null>(null);
  const toast = useToast();

  const startCapture = useCallback(() => {
    setIsCapturing(true);
    toast.info("Capturing screenshot…", "Rendering all messages");
  }, [toast]);

  const onCaptureComplete = useCallback(
    (result?: Blob, error?: Error) => {
      setIsCapturing(false);
      if (error || !result) {
        toast.error("Screenshot failed", error?.message ?? "Unknown error");
        return;
      }
      const url = URL.createObjectURL(result);
      objectUrlRef.current = url;
      setScreenshot({ blob: result, url });
    },
    [toast]
  );

  const dismissPreview = useCallback(() => {
    setScreenshot(null);
    if (objectUrlRef.current) {
      URL.revokeObjectURL(objectUrlRef.current);
      objectUrlRef.current = null;
    }
  }, []);

  return { isCapturing, screenshot, startCapture, onCaptureComplete, dismissPreview } as const;
}
