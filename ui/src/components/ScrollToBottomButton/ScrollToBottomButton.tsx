import { memo } from "react";
import { ChevronDown } from "lucide-react";
import { Button } from "@/components/Button/Button";
import { cn } from "@/utils/cn";

interface ScrollToBottomButtonProps {
  /** Whether the button should be visible (user has scrolled up) */
  visible: boolean;
  /** Callback to scroll to bottom */
  onClick: () => void;
  /** Optional className for positioning */
  className?: string;
}

/**
 * ScrollToBottomButton - Floating button to scroll chat to bottom
 *
 * Appears when user scrolls up in the chat, allowing them to quickly
 * return to the latest messages. Uses fade-in/fade-out animation.
 */
export const ScrollToBottomButton = memo(function ScrollToBottomButton({
  visible,
  onClick,
  className,
}: ScrollToBottomButtonProps) {
  return (
    <div
      className={cn(
        "transition-all duration-200 ease-in-out",
        visible ? "opacity-100 translate-y-0" : "opacity-0 translate-y-2 pointer-events-none",
        className
      )}
    >
      <Button
        variant="secondary"
        size="icon"
        onClick={onClick}
        aria-label="Scroll to bottom"
        className="shadow-lg hover:shadow-xl"
      >
        <ChevronDown className="h-5 w-5" />
      </Button>
    </div>
  );
});
