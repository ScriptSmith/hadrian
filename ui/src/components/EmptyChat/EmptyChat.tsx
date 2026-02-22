import { useState } from "react";
import { Sparkles } from "lucide-react";

import { cn } from "@/utils/cn";
import { useChatUIStore } from "@/stores/chatUIStore";
import { EXAMPLE_PROMPT_CATEGORIES, type PromptCategory } from "./examplePrompts";

interface EmptyChatProps {
  selectedModels: string[];
  isLoadingModels?: boolean;
}

export function EmptyChat({ selectedModels, isLoadingModels = false }: EmptyChatProps) {
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const setPendingPrompt = useChatUIStore((s) => s.setPendingPrompt);

  const getMessage = () => {
    if (selectedModels.length === 0) {
      return "Select a model to start chatting.";
    }
    if (selectedModels.length === 1) {
      const modelName = selectedModels[0].split("/").pop();
      return `Start a conversation with ${modelName}.`;
    }
    return `Compare responses from ${selectedModels.length} models.`;
  };

  const activeCategory = selectedCategory
    ? EXAMPLE_PROMPT_CATEGORIES.find((c) => c.id === selectedCategory)
    : null;

  const handlePromptSelect = (prompt: string) => {
    setPendingPrompt(prompt);
  };

  return (
    <div className="flex h-full flex-col items-center justify-center text-center animate-slide-up-bounce px-4">
      <div className="mb-4 sm:mb-6 flex h-16 w-16 sm:h-20 sm:w-20 items-center justify-center rounded-2xl bg-gradient-to-br from-primary/20 to-primary/5">
        <Sparkles className="h-8 w-8 sm:h-10 sm:w-10 text-primary" />
      </div>
      <h2 className="text-xl sm:text-2xl font-semibold">How can I help you today?</h2>
      <p className="mt-2 sm:mt-3 max-w-md text-sm sm:text-base text-muted-foreground">
        {getMessage()}
      </p>
      {isLoadingModels && (
        <p className="mt-3 sm:mt-4 text-xs sm:text-sm text-muted-foreground">
          Loading available models...
        </p>
      )}

      {/* Example prompts section */}
      {!isLoadingModels && (
        <div className="mt-8 w-full max-w-2xl">
          {/* Category tabs */}
          <div className="flex flex-wrap justify-center gap-2 mb-4">
            {EXAMPLE_PROMPT_CATEGORIES.map((category) => (
              <CategoryTab
                key={category.id}
                category={category}
                isSelected={selectedCategory === category.id}
                onClick={() =>
                  setSelectedCategory(selectedCategory === category.id ? null : category.id)
                }
              />
            ))}
          </div>

          {/* Prompts for selected category */}
          {activeCategory && (
            <div className="grid gap-2 sm:grid-cols-2 text-left animate-in fade-in slide-in-from-bottom-2 duration-200">
              {activeCategory.prompts.map((prompt, index) => (
                <button
                  key={index}
                  onClick={() => handlePromptSelect(prompt.prompt)}
                  className="group p-3 rounded-lg border border-border/50 bg-card/50 hover:bg-accent hover:border-border transition-colors text-left"
                >
                  <div className="flex items-start gap-2">
                    <activeCategory.icon
                      className={cn("h-4 w-4 mt-0.5 shrink-0", activeCategory.color)}
                    />
                    <div className="min-w-0">
                      <div className="font-medium text-sm group-hover:text-accent-foreground">
                        {prompt.title}
                      </div>
                      <div className="text-xs text-muted-foreground line-clamp-2 mt-0.5">
                        {prompt.prompt.slice(0, 80)}...
                      </div>
                    </div>
                  </div>
                </button>
              ))}
            </div>
          )}

          {/* Hint when no category selected */}
          {!activeCategory && (
            <p className="text-xs text-muted-foreground">
              Click a category above to see example prompts
            </p>
          )}
        </div>
      )}
    </div>
  );
}

interface CategoryTabProps {
  category: PromptCategory;
  isSelected: boolean;
  onClick: () => void;
}

function CategoryTab({ category, isSelected, onClick }: CategoryTabProps) {
  const Icon = category.icon;

  return (
    <button
      onClick={onClick}
      className={cn(
        "flex items-center gap-1.5 px-3 py-1.5 rounded-full text-sm font-medium transition-colors",
        isSelected
          ? "bg-primary text-primary-foreground"
          : "bg-muted/50 text-muted-foreground hover:bg-muted hover:text-foreground"
      )}
    >
      <Icon className={cn("h-3.5 w-3.5", isSelected ? "" : category.color)} />
      {category.name}
    </button>
  );
}
