import { useState } from "react";
import { X } from "lucide-react";

const STORAGE_KEY = "hadrian-alpha-banner-dismissed";

export function AlphaBanner() {
  const [dismissed, setDismissed] = useState(() => localStorage.getItem(STORAGE_KEY) === "true");

  if (dismissed) return null;

  const handleDismiss = () => {
    localStorage.setItem(STORAGE_KEY, "true");
    setDismissed(true);
  };

  return (
    <div
      role="status"
      className="relative flex items-center justify-center gap-2 bg-amber-500 px-4 py-1.5 text-sm font-medium text-black dark:bg-amber-400"
    >
      <span>Hadrian is experimental alpha software. Do not use in production.</span>
      <button
        onClick={handleDismiss}
        className="absolute right-2 rounded-sm p-0.5 hover:bg-black/10"
        aria-label="Dismiss banner"
      >
        <X className="h-4 w-4" />
      </button>
    </div>
  );
}
