import { snapdom } from "@zumer/snapdom";

export interface CaptureOptions {
  /** Device pixel ratio for the capture (default: 2 for retina) */
  scale?: number;
  /** Background color override; auto-resolved from CSS --color-background if omitted */
  backgroundColor?: string;
}

/**
 * Capture a DOM element as a PNG blob.
 */
export async function captureElementAsBlob(
  element: HTMLElement,
  options: CaptureOptions = {}
): Promise<Blob> {
  const { scale = 2, backgroundColor } = options;

  const bg =
    backgroundColor ??
    getComputedStyle(document.documentElement).getPropertyValue("--color-background").trim();

  const snapshot = await snapdom(element, {
    scale,
    backgroundColor: bg || undefined,
  });
  return snapshot.toBlob({ type: "png" });
}

/**
 * Generate a filename for a screenshot export, matching the pattern used for other exports.
 */
export function generateScreenshotFilename(title: string): string {
  const sanitized = title
    .replace(/[^a-zA-Z0-9\s-]/g, "")
    .replace(/\s+/g, "-")
    .toLowerCase()
    .slice(0, 50);

  const timestamp = new Date().toISOString().slice(0, 10);
  return `${sanitized}-${timestamp}.png`;
}

export function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
}
