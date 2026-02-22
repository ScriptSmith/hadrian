import {
  useState,
  useEffect,
  useRef,
  useCallback,
  createContext,
  useContext,
  type ReactNode,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { createPortal } from "react-dom";
import { Search, Command as CommandIcon } from "lucide-react";
import { cn } from "@/utils/cn";

interface CommandItem {
  id: string;
  label: string;
  description?: string;
  icon?: ReactNode;
  shortcut?: string[];
  onSelect: () => void;
  category?: string;
}

interface CommandPaletteContextValue {
  open: boolean;
  setOpen: (open: boolean) => void;
  registerCommand: (command: CommandItem) => void;
  unregisterCommand: (id: string) => void;
}

const CommandPaletteContext = createContext<CommandPaletteContextValue | null>(null);

export function useCommandPalette() {
  const context = useContext(CommandPaletteContext);
  if (!context) {
    throw new Error("useCommandPalette must be used within CommandPaletteProvider");
  }
  return context;
}

interface CommandPaletteProviderProps {
  children: ReactNode;
}

export function CommandPaletteProvider({ children }: CommandPaletteProviderProps) {
  const [open, setOpen] = useState(false);
  const [commands, setCommands] = useState<Map<string, CommandItem>>(new Map());

  const registerCommand = useCallback((command: CommandItem) => {
    setCommands((prev) => new Map(prev).set(command.id, command));
  }, []);

  const unregisterCommand = useCallback((id: string) => {
    setCommands((prev) => {
      const next = new Map(prev);
      next.delete(id);
      return next;
    });
  }, []);

  // Global keyboard shortcut
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        setOpen((prev) => !prev);
      }
      if (e.key === "Escape" && open) {
        setOpen(false);
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [open]);

  return (
    <CommandPaletteContext.Provider value={{ open, setOpen, registerCommand, unregisterCommand }}>
      {children}
      {open && <CommandPaletteDialog commands={commands} onClose={() => setOpen(false)} />}
    </CommandPaletteContext.Provider>
  );
}

interface CommandPaletteDialogProps {
  commands: Map<string, CommandItem>;
  onClose: () => void;
}

function CommandPaletteDialog({ commands, onClose }: CommandPaletteDialogProps) {
  const [search, setSearch] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // Filter commands based on search
  const filteredCommands = Array.from(commands.values()).filter((cmd) => {
    const query = search.toLowerCase();
    return (
      cmd.label.toLowerCase().includes(query) ||
      cmd.description?.toLowerCase().includes(query) ||
      cmd.category?.toLowerCase().includes(query)
    );
  });

  // Group commands by category
  const groupedCommands = filteredCommands.reduce<Map<string, CommandItem[]>>((acc, cmd) => {
    const category = cmd.category || "Actions";
    if (!acc.has(category)) {
      acc.set(category, []);
    }
    acc.get(category)!.push(cmd);
    return acc;
  }, new Map());

  // Flatten for keyboard navigation
  const flatCommands = Array.from(groupedCommands.values()).flat();

  // Handle search input change and reset selection
  const handleSearchChange = useCallback((value: string) => {
    setSearch(value);
    setSelectedIndex(0);
  }, []);

  // Focus input on open
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Scroll selected item into view
  useEffect(() => {
    const selectedElement = listRef.current?.querySelector(`[data-index="${selectedIndex}"]`);
    selectedElement?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  const handleKeyDown = (e: ReactKeyboardEvent) => {
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        setSelectedIndex((prev) => (prev + 1) % flatCommands.length);
        break;
      case "ArrowUp":
        e.preventDefault();
        setSelectedIndex((prev) => (prev - 1 + flatCommands.length) % flatCommands.length);
        break;
      case "Enter":
        e.preventDefault();
        if (flatCommands[selectedIndex]) {
          flatCommands[selectedIndex].onSelect();
          onClose();
        }
        break;
      case "Escape":
        e.preventDefault();
        onClose();
        break;
    }
  };

  const handleSelect = (command: CommandItem) => {
    command.onSelect();
    onClose();
  };

  return createPortal(
    <>
      {/* Backdrop */}
      <div
        className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm animate-in fade-in-0"
        onClick={onClose}
        aria-hidden="true"
      />

      {/* Dialog */}
      <div className="fixed left-1/2 top-[20%] z-50 w-full max-w-lg -translate-x-1/2 animate-in fade-in-0 zoom-in-95 slide-in-from-top-4">
        <div className="overflow-hidden rounded-xl border bg-popover shadow-2xl ring-1 ring-black/5">
          {/* Search input */}
          <div className="flex items-center border-b px-4">
            <Search className="h-5 w-5 shrink-0 text-muted-foreground" aria-hidden="true" />
            <input
              ref={inputRef}
              type="text"
              value={search}
              onChange={(e) => handleSearchChange(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Type a command or search..."
              aria-label="Search commands"
              className="flex-1 bg-transparent px-4 py-4 text-sm outline-none placeholder:text-muted-foreground"
            />
            <kbd className="pointer-events-none hidden h-6 select-none items-center gap-1 rounded border bg-muted px-1.5 font-mono text-xs text-muted-foreground sm:flex">
              <span className="text-xs">ESC</span>
            </kbd>
          </div>

          {/* Commands list */}
          <div ref={listRef} className="max-h-[300px] overflow-y-auto p-2">
            {flatCommands.length === 0 ? (
              <div className="py-6 text-center text-sm text-muted-foreground">
                No commands found
              </div>
            ) : (
              Array.from(groupedCommands.entries()).map(([category, items]) => (
                <div key={category}>
                  <div className="px-2 py-1.5 text-xs font-semibold text-muted-foreground">
                    {category}
                  </div>
                  {items.map((cmd) => {
                    const index = flatCommands.indexOf(cmd);
                    const isSelected = index === selectedIndex;
                    return (
                      <button
                        key={cmd.id}
                        data-index={index}
                        className={cn(
                          "flex w-full items-center gap-3 rounded-md px-3 py-2 text-left text-sm transition-colors",
                          isSelected ? "bg-accent text-accent-foreground" : "hover:bg-accent/50"
                        )}
                        onClick={() => handleSelect(cmd)}
                        onMouseEnter={() => setSelectedIndex(index)}
                      >
                        {cmd.icon && (
                          <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md bg-muted">
                            {cmd.icon}
                          </span>
                        )}
                        <div className="flex flex-1 flex-col">
                          <span className="font-medium">{cmd.label}</span>
                          {cmd.description && (
                            <span className="text-xs text-muted-foreground">{cmd.description}</span>
                          )}
                        </div>
                        {cmd.shortcut && (
                          <div className="flex items-center gap-1">
                            {cmd.shortcut.map((key, i) => (
                              <kbd
                                key={i}
                                className="pointer-events-none h-5 select-none items-center gap-1 rounded border bg-muted px-1.5 font-mono text-[10px] text-muted-foreground"
                              >
                                {key}
                              </kbd>
                            ))}
                          </div>
                        )}
                      </button>
                    );
                  })}
                </div>
              ))
            )}
          </div>

          {/* Footer */}
          <div className="flex items-center justify-between border-t px-4 py-2 text-xs text-muted-foreground">
            <div className="flex items-center gap-4">
              <span className="flex items-center gap-1">
                <kbd className="h-4 rounded border bg-muted px-1 text-[10px]">↑</kbd>
                <kbd className="h-4 rounded border bg-muted px-1 text-[10px]">↓</kbd>
                to navigate
              </span>
              <span className="flex items-center gap-1">
                <kbd className="h-4 rounded border bg-muted px-1 text-[10px]">↵</kbd>
                to select
              </span>
            </div>
            <div className="flex items-center gap-1">
              <CommandIcon className="h-3 w-3" />
              <span>K to toggle</span>
            </div>
          </div>
        </div>
      </div>
    </>,
    document.body
  );
}

// Hook to register commands
export function useRegisterCommand(command: Omit<CommandItem, "id"> & { id?: string }) {
  const { registerCommand, unregisterCommand } = useCommandPalette();
  const idRef = useRef(command.id || crypto.randomUUID());

  useEffect(() => {
    const id = idRef.current;
    const cmd: CommandItem = {
      ...command,
      id,
    };
    registerCommand(cmd);
    return () => unregisterCommand(id);
  }, [command, registerCommand, unregisterCommand]);
}
