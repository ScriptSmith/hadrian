import { memo } from "react";
import { ChevronUp } from "lucide-react";
import { Button } from "@/components/Button/Button";
import { cn } from "@/utils/cn";

interface ScrollToPreviousButtonProps {
  /** Whether the button should be visible (user has scrolled up and there are previous messages) */
  visible: boolean;
  /** Callback to scroll to previous message group */
  onClick: () => void;
  /** Optional className for positioning */
  className?: string;
}

/**
 * ScrollToPreviousButton - Floating button to scroll to previous message
 *
 * Appears when user scrolls up in the chat, allowing them to quickly
 * navigate to the previous message group. Uses fade-in/fade-out animation.
 */
export const ScrollToPreviousButton = memo(function ScrollToPreviousButton({
  visible,
  onClick,
  className,
}: ScrollToPreviousButtonProps) {
  return (
    <div
      className={cn(
        "transition-all duration-200 ease-in-out",
        visible ? "opacity-100 translate-y-0" : "opacity-0 -translate-y-2 pointer-events-none",
        className
      )}
    >
      <Button
        variant="secondary"
        size="icon"
        onClick={onClick}
        aria-label="Scroll to previous message"
        className="shadow-lg hover:shadow-xl"
      >
        <ChevronUp className="h-5 w-5" />
      </Button>
    </div>
  );
});
