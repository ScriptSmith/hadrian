import { useRef, useEffect, useState, useCallback, type KeyboardEvent } from "react";
import { Image, Volume2, Film } from "lucide-react";
import { cn } from "@/utils/cn";
import type { StudioTab } from "@/pages/studio/types";

const tabs: { id: StudioTab; label: string; icon: typeof Image }[] = [
  { id: "images", label: "Images", icon: Image },
  { id: "audio", label: "Audio", icon: Volume2 },
  { id: "video", label: "Video", icon: Film },
];

interface StudioTabsProps {
  activeTab: StudioTab;
  onTabChange: (tab: StudioTab) => void;
}

export function StudioTabs({ activeTab, onTabChange }: StudioTabsProps) {
  const tabRefs = useRef<Map<StudioTab, HTMLButtonElement>>(new Map());
  const containerRef = useRef<HTMLDivElement>(null);
  const [indicator, setIndicator] = useState({ left: 0, width: 0 });

  const updateIndicator = useCallback(() => {
    const el = tabRefs.current.get(activeTab);
    const container = containerRef.current;
    if (el && container) {
      const containerRect = container.getBoundingClientRect();
      const tabRect = el.getBoundingClientRect();
      setIndicator({
        left: tabRect.left - containerRect.left,
        width: tabRect.width,
      });
    }
  }, [activeTab]);

  useEffect(() => {
    updateIndicator();
    window.addEventListener("resize", updateIndicator);
    return () => window.removeEventListener("resize", updateIndicator);
  }, [updateIndicator]);

  const handleKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const currentIndex = tabs.findIndex((t) => t.id === activeTab);
    let nextIndex = currentIndex;

    if (e.key === "ArrowLeft") {
      e.preventDefault();
      nextIndex = currentIndex > 0 ? currentIndex - 1 : tabs.length - 1;
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      nextIndex = currentIndex < tabs.length - 1 ? currentIndex + 1 : 0;
    } else if (e.key === "Home") {
      e.preventDefault();
      nextIndex = 0;
    } else if (e.key === "End") {
      e.preventDefault();
      nextIndex = tabs.length - 1;
    } else {
      return;
    }

    const nextTab = tabs[nextIndex];
    onTabChange(nextTab.id);
    tabRefs.current.get(nextTab.id)?.focus();
  };

  return (
    <div
      ref={containerRef}
      role="tablist"
      aria-label="Studio sections"
      className="relative flex border-b border-border"
      onKeyDown={handleKeyDown}
      tabIndex={-1}
    >
      {tabs.map((tab) => {
        const isActive = tab.id === activeTab;
        const Icon = tab.icon;
        return (
          <button
            key={tab.id}
            ref={(el) => {
              if (el) tabRefs.current.set(tab.id, el);
            }}
            role="tab"
            id={`studio-tab-${tab.id}`}
            aria-selected={isActive}
            aria-controls={`studio-panel-${tab.id}`}
            tabIndex={isActive ? 0 : -1}
            className={cn(
              "relative flex items-center gap-2 px-5 py-3 text-sm font-medium transition-colors",
              "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset",
              "motion-safe:transition-colors motion-safe:duration-200",
              isActive ? "text-foreground" : "text-muted-foreground hover:text-foreground/80"
            )}
            onClick={() => onTabChange(tab.id)}
          >
            <Icon className="h-4 w-4" aria-hidden="true" />
            <span className="hidden sm:inline">{tab.label}</span>
          </button>
        );
      })}

      {/* Animated underline indicator */}
      <div
        className="absolute bottom-0 h-0.5 bg-primary motion-safe:transition-all motion-safe:duration-300 motion-safe:ease-out"
        style={{ left: indicator.left, width: indicator.width }}
        aria-hidden="true"
      />
    </div>
  );
}
