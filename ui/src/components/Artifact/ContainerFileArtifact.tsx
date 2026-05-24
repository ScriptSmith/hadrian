/**
 * ContainerFileArtifact - Renders a file the shell tool wrote to /mnt/data.
 *
 * The bytes aren't inlined in the artifact; we lazily fetch them from the
 * authed container content endpoint. Images render inline (with download);
 * everything else renders as a download chip. The `<img>` element can't send
 * the bearer token, so images go through an authed fetch → object URL.
 */

import { memo, useState, useEffect, useCallback } from "react";
import { Download, FileText, Loader2 } from "lucide-react";

import type { Artifact, ContainerFileArtifactData } from "@/components/chat-types";
import { Button } from "@/components/Button/Button";
import { useAuth } from "@/auth";
import { formatBytes } from "@/utils/formatters";
import { formatApiError } from "@/utils/formatApiError";

export interface ContainerFileArtifactProps {
  artifact: Artifact;
  className?: string;
}

function isContainerFileData(data: unknown): data is ContainerFileArtifactData {
  return (
    typeof data === "object" &&
    data !== null &&
    typeof (data as ContainerFileArtifactData).containerId === "string" &&
    typeof (data as ContainerFileArtifactData).fileId === "string"
  );
}

function ContainerFileArtifactComponent({ artifact }: ContainerFileArtifactProps) {
  const { token } = useAuth();
  const data = isContainerFileData(artifact.data) ? artifact.data : null;
  const isImage = !!data?.contentType?.startsWith("image/");

  const [objectUrl, setObjectUrl] = useState<string | null>(null);
  const [downloading, setDownloading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const contentUrl = data
    ? `/api/v1/containers/${data.containerId}/files/${data.fileId}/content`
    : "";

  const fetchBlob = useCallback(async (): Promise<Blob> => {
    const res = await fetch(contentUrl, {
      headers: token ? { Authorization: `Bearer ${token}` } : {},
    });
    if (!res.ok) throw new Error(`Failed to load file (${res.status})`);
    return res.blob();
  }, [contentUrl, token]);

  // Eagerly load images so they render inline; other files load on download.
  useEffect(() => {
    if (!data || !isImage) return;
    let url: string | null = null;
    let cancelled = false;
    (async () => {
      try {
        const blob = await fetchBlob();
        if (cancelled) return;
        url = URL.createObjectURL(blob);
        setObjectUrl(url);
      } catch (err) {
        if (!cancelled) setError(formatApiError(err));
      }
    })();
    return () => {
      cancelled = true;
      if (url) URL.revokeObjectURL(url);
    };
  }, [data, isImage, fetchBlob]);

  const handleDownload = useCallback(async () => {
    if (!data) return;
    setDownloading(true);
    try {
      const blob = await fetchBlob();
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = data.filename;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    } catch (err) {
      setError(formatApiError(err));
    } finally {
      setDownloading(false);
    }
  }, [data, fetchBlob]);

  if (!data) {
    return <div className="p-4 text-sm text-muted-foreground">Invalid container file artifact</div>;
  }

  if (isImage) {
    return (
      <div className="relative group p-2">
        {error ? (
          <div className="p-4 text-sm text-muted-foreground">{error}</div>
        ) : objectUrl ? (
          <>
            <Button
              variant="secondary"
              size="sm"
              className="absolute right-3 top-3 h-7 w-7 p-0 opacity-0 group-hover:opacity-100 transition-opacity"
              onClick={handleDownload}
              aria-label={`Download ${data.filename}`}
            >
              <Download className="h-3.5 w-3.5" />
            </Button>
            <img
              src={objectUrl}
              alt={data.filename}
              className="rounded max-w-full max-h-[400px] mx-auto"
            />
            <p className="mt-1 text-center text-xs text-muted-foreground font-mono">
              {data.filename}
            </p>
          </>
        ) : (
          <div className="flex items-center justify-center gap-2 p-6 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            Loading {data.filename}…
          </div>
        )}
      </div>
    );
  }

  // Non-image: download chip.
  return (
    <div className="flex items-center gap-3 p-3">
      <FileText className="h-5 w-5 text-muted-foreground shrink-0" />
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium truncate font-mono">{data.filename}</p>
        <p className="text-xs text-muted-foreground">
          {data.contentType ?? "file"}
          {typeof data.bytes === "number" ? ` · ${formatBytes(data.bytes)}` : ""}
        </p>
        {error && <p className="text-xs text-destructive mt-0.5">{error}</p>}
      </div>
      <Button
        variant="outline"
        size="sm"
        onClick={handleDownload}
        disabled={downloading}
        aria-label={`Download ${data.filename}`}
      >
        {downloading ? (
          <Loader2 className="h-4 w-4 animate-spin" />
        ) : (
          <Download className="h-4 w-4" />
        )}
        <span className="ml-1.5">Download</span>
      </Button>
    </div>
  );
}

export const ContainerFileArtifact = memo(ContainerFileArtifactComponent);
