import { Film } from "lucide-react";
import { Button } from "@/components/Button/Button";

interface VideoPlaceholderProps {
  onNavigateToImages: () => void;
}

export function VideoPlaceholder({ onNavigateToImages }: VideoPlaceholderProps) {
  return (
    <div className="flex h-full flex-col items-center justify-center py-20">
      {/* Animated gradient background behind icon */}
      <div className="relative mb-6">
        <div className="absolute -inset-4 rounded-3xl bg-gradient-to-br from-primary/20 via-primary/5 to-primary/20 blur-xl motion-safe:animate-pulse" />
        <div className="relative flex h-20 w-20 items-center justify-center rounded-2xl bg-muted/50 backdrop-blur-sm">
          <Film className="h-10 w-10 text-muted-foreground" />
        </div>
      </div>

      <h2 className="text-xl font-semibold text-foreground">Coming Soon</h2>
      <p className="mt-2 max-w-sm text-center text-sm text-muted-foreground">
        Video generation will be available in a future release.
      </p>

      <Button variant="outline" className="mt-6" onClick={onNavigateToImages}>
        Explore Images
      </Button>
    </div>
  );
}
