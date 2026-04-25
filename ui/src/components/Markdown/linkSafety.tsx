import { useCallback, useEffect, useState } from "react";
import { Check, Copy, ExternalLink } from "lucide-react";
import { Modal, ModalClose } from "@/components/Modal/Modal";
import { Button } from "@/components/Button/Button";
import { cn } from "@/utils/cn";

interface LinkSafetyModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: () => void;
  url: string;
}

const TRUSTED_DOMAINS_KEY = "hadrian:trusted-link-domains";

function getDomain(url: string): string | null {
  try {
    const host = new URL(url).hostname;
    return host || null;
  } catch {
    return null;
  }
}

function getTrustedDomains(): string[] {
  try {
    const raw = window.localStorage.getItem(TRUSTED_DOMAINS_KEY);
    return raw ? (JSON.parse(raw) as string[]) : [];
  } catch {
    return [];
  }
}

function addTrustedDomain(domain: string) {
  try {
    const current = getTrustedDomains();
    if (current.includes(domain)) return;
    window.localStorage.setItem(TRUSTED_DOMAINS_KEY, JSON.stringify([...current, domain]));
  } catch {
    // localStorage may be unavailable (private browsing, quota); ignore.
  }
}

function isTrustedUrl(url: string): boolean {
  const domain = getDomain(url);
  return domain !== null && getTrustedDomains().includes(domain);
}

// Streamdown's default link-safety modal renders inline inside the markdown
// container. When the markdown lives in a scrollable region (like the chat
// history with overflow-y-auto), wheel events still bubble to that ancestor
// and scroll it behind the modal. Our Modal portals to document.body, which
// escapes the scrollable ancestor entirely.
function LinkSafetyModal({ isOpen, onClose, onConfirm, url }: LinkSafetyModalProps) {
  const [copied, setCopied] = useState(false);
  const [alwaysAllow, setAlwaysAllow] = useState(false);
  const domain = getDomain(url);

  // Reset the checkbox each time a new modal opens.
  useEffect(() => {
    if (isOpen) setAlwaysAllow(false);
  }, [isOpen, url]);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(url);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard write can fail without focus or permission; ignore.
    }
  }, [url]);

  const handleConfirm = useCallback(() => {
    if (alwaysAllow && domain) {
      addTrustedDomain(domain);
    }
    onConfirm();
    onClose();
  }, [alwaysAllow, domain, onClose, onConfirm]);

  return (
    <Modal open={isOpen} onClose={onClose}>
      <ModalClose onClose={onClose} />
      <div className="flex flex-col gap-4">
        <div className="flex flex-col gap-2 pr-8">
          <div className="flex items-center gap-2 text-lg font-semibold">
            <ExternalLink className="h-5 w-5" aria-hidden="true" />
            <span>Open external link?</span>
          </div>
          <p className="text-sm text-muted-foreground">
            You&apos;re about to visit an external website.
          </p>
        </div>
        <div
          className={cn(
            "break-all rounded-md bg-muted p-3 font-mono text-sm",
            url.length > 100 && "max-h-32 overflow-y-auto"
          )}
        >
          {url}
        </div>
        {domain && (
          <label className="flex cursor-pointer items-center gap-2 text-sm text-muted-foreground">
            <input
              type="checkbox"
              checked={alwaysAllow}
              onChange={(e) => setAlwaysAllow(e.target.checked)}
              className="h-4 w-4 cursor-pointer rounded border-input accent-primary"
            />
            <span>
              Always allow links from <span className="font-mono text-foreground">{domain}</span>
            </span>
          </label>
        )}
        <div className="flex gap-2">
          <Button type="button" variant="outline" onClick={handleCopy} className="flex-1 gap-2">
            {copied ? (
              <>
                <Check className="h-3.5 w-3.5" aria-hidden="true" />
                <span>Copied</span>
              </>
            ) : (
              <>
                <Copy className="h-3.5 w-3.5" aria-hidden="true" />
                <span>Copy link</span>
              </>
            )}
          </Button>
          <Button type="button" variant="primary" onClick={handleConfirm} className="flex-1 gap-2">
            <ExternalLink className="h-3.5 w-3.5" aria-hidden="true" />
            <span>Open link</span>
          </Button>
        </div>
      </div>
    </Modal>
  );
}

export const linkSafety = {
  enabled: true,
  onLinkCheck: (url: string) => isTrustedUrl(url),
  renderModal: (props: LinkSafetyModalProps) => <LinkSafetyModal {...props} />,
};
