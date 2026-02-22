import {
  useState,
  useRef,
  useEffect,
  useCallback,
  useId,
  type ReactNode,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { createPortal } from "react-dom";
import { ChevronDown, Check, X } from "lucide-react";
import { cn } from "@/utils/cn";

export interface SelectOption<T = string> {
  value: T;
  label: string;
  disabled?: boolean;
}

interface SelectBaseProps<T> {
  options: SelectOption<T>[];
  placeholder?: string;
  disabled?: boolean;
  error?: boolean;
  className?: string;
  searchable?: boolean;
  /** Whether the selection can be cleared. Defaults to true. */
  clearable?: boolean;
}

interface SingleSelectProps<T> extends SelectBaseProps<T> {
  multiple?: false;
  value: T | null;
  onChange: (value: T | null) => void;
}

interface MultiSelectProps<T> extends SelectBaseProps<T> {
  multiple: true;
  value: T[];
  onChange: (value: T[]) => void;
}

type SelectProps<T> = SingleSelectProps<T> | MultiSelectProps<T>;

export function Select<T extends string | number>({
  options,
  placeholder = "Select...",
  disabled,
  error,
  className,
  searchable = false,
  clearable = true,
  ...props
}: SelectProps<T>) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");
  const [highlightedIndex, setHighlightedIndex] = useState(-1);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const optionRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const [position, setPosition] = useState({ top: 0, left: 0, width: 0 });
  const listboxId = useId();

  const isMultiple = "multiple" in props && props.multiple === true;
  const value = props.value;
  const onChange = props.onChange;

  const selectedOptions = isMultiple
    ? options.filter((o) => (value as T[]).includes(o.value))
    : options.find((o) => o.value === value);

  const filteredOptions = searchable
    ? options.filter((o) => o.label.toLowerCase().includes(search.toLowerCase()))
    : options;

  const updatePosition = useCallback(() => {
    if (triggerRef.current) {
      const rect = triggerRef.current.getBoundingClientRect();
      setPosition({
        top: rect.bottom + 4,
        left: rect.left,
        width: rect.width,
      });
    }
  }, []);

  useEffect(() => {
    if (open) {
      updatePosition();
      setHighlightedIndex(-1);
      window.addEventListener("resize", updatePosition);
      window.addEventListener("scroll", updatePosition, true);
      if (searchable && inputRef.current) {
        inputRef.current.focus();
      }
    }
    return () => {
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
    };
  }, [open, updatePosition, searchable]);

  // Reset option refs when filtered options change
  useEffect(() => {
    optionRefs.current = optionRefs.current.slice(0, filteredOptions.length);
  }, [filteredOptions.length]);

  // Focus highlighted option
  useEffect(() => {
    if (highlightedIndex >= 0 && optionRefs.current[highlightedIndex]) {
      optionRefs.current[highlightedIndex]?.focus();
    }
  }, [highlightedIndex]);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (
        contentRef.current &&
        !contentRef.current.contains(e.target as Node) &&
        triggerRef.current &&
        !triggerRef.current.contains(e.target as Node)
      ) {
        setOpen(false);
        setSearch("");
        triggerRef.current?.focus();
      }
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      if (!open) return;

      switch (e.key) {
        case "Escape":
          e.preventDefault();
          setOpen(false);
          setSearch("");
          triggerRef.current?.focus();
          break;
        case "ArrowDown":
          e.preventDefault();
          setHighlightedIndex((prev) => {
            const nextIndex = prev < filteredOptions.length - 1 ? prev + 1 : 0;
            // Skip disabled options
            let index = nextIndex;
            while (filteredOptions[index]?.disabled && index !== prev) {
              index = index < filteredOptions.length - 1 ? index + 1 : 0;
            }
            return index;
          });
          break;
        case "ArrowUp":
          e.preventDefault();
          setHighlightedIndex((prev) => {
            const nextIndex = prev > 0 ? prev - 1 : filteredOptions.length - 1;
            // Skip disabled options
            let index = nextIndex;
            while (filteredOptions[index]?.disabled && index !== prev) {
              index = index > 0 ? index - 1 : filteredOptions.length - 1;
            }
            return index;
          });
          break;
        case "Home":
          e.preventDefault();
          setHighlightedIndex(0);
          break;
        case "End":
          e.preventDefault();
          setHighlightedIndex(filteredOptions.length - 1);
          break;
        case "Enter":
          if (highlightedIndex >= 0 && filteredOptions[highlightedIndex]) {
            e.preventDefault();
            handleSelect(filteredOptions[highlightedIndex]);
          }
          break;
        case "Tab":
          setOpen(false);
          setSearch("");
          break;
      }
    };

    if (open) {
      document.addEventListener("mousedown", handleClickOutside);
      document.addEventListener("keydown", handleKeyDown);
    }

    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("keydown", handleKeyDown);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, highlightedIndex, filteredOptions]);

  const handleSelect = (option: SelectOption<T>) => {
    if (option.disabled) return;

    if (isMultiple) {
      const currentValue = value as T[];
      const newValue = currentValue.includes(option.value)
        ? currentValue.filter((v) => v !== option.value)
        : [...currentValue, option.value];
      (onChange as (value: T[]) => void)(newValue);
    } else {
      (onChange as (value: T | null) => void)(option.value);
      setOpen(false);
      setSearch("");
      triggerRef.current?.focus();
    }
  };

  const handleClear = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (isMultiple) {
      (onChange as (value: T[]) => void)([]);
    } else {
      (onChange as (value: T | null) => void)(null);
    }
  };

  const handleTriggerKeyDown = (e: ReactKeyboardEvent<HTMLButtonElement>) => {
    switch (e.key) {
      case "ArrowDown":
      case "ArrowUp":
        e.preventDefault();
        setOpen(true);
        setHighlightedIndex(e.key === "ArrowDown" ? 0 : filteredOptions.length - 1);
        break;
      case "Enter":
      case " ":
        if (!open) {
          e.preventDefault();
          setOpen(true);
          setHighlightedIndex(0);
        }
        break;
    }
  };

  const displayValue = (): ReactNode => {
    if (isMultiple) {
      const selected = selectedOptions as SelectOption<T>[];
      if (selected.length === 0) return placeholder;
      if (selected.length === 1) return selected[0].label;
      return `${selected.length} selected`;
    }
    return (selectedOptions as SelectOption<T> | undefined)?.label ?? placeholder;
  };

  const hasValue = isMultiple ? (value as T[]).length > 0 : value !== null;

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        disabled={disabled}
        className={cn(
          "flex h-10 w-full items-center justify-between rounded-md border border-input bg-background px-3 py-2 text-sm",
          "ring-offset-background placeholder:text-muted-foreground",
          "focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
          "disabled:cursor-not-allowed disabled:opacity-50",
          error && "border-destructive focus:ring-destructive",
          !hasValue && "text-muted-foreground",
          className
        )}
        onClick={() => setOpen(!open)}
        onKeyDown={handleTriggerKeyDown}
        aria-expanded={open}
        aria-haspopup="listbox"
        aria-controls={open ? listboxId : undefined}
      >
        <span className="truncate">{displayValue()}</span>
        <div className="flex items-center gap-1">
          {hasValue && !disabled && clearable && (
            <X
              className="h-4 w-4 opacity-50 hover:opacity-100"
              onClick={handleClear}
              aria-label="Clear selection"
            />
          )}
          <ChevronDown
            className={cn("h-4 w-4 opacity-50 transition-transform", open && "rotate-180")}
            aria-hidden="true"
          />
        </div>
      </button>

      {open &&
        createPortal(
          <div
            ref={contentRef}
            id={listboxId}
            role="listbox"
            aria-multiselectable={isMultiple || undefined}
            tabIndex={-1}
            className="fixed z-50 overflow-hidden rounded-md border bg-popover text-popover-foreground shadow-md animate-in fade-in-0 zoom-in-95"
            style={{
              top: position.top,
              left: position.left,
              width: position.width,
              maxHeight: 300,
            }}
          >
            {searchable && (
              <div className="border-b p-2">
                <input
                  ref={inputRef}
                  type="text"
                  className="w-full bg-transparent text-sm outline-none placeholder:text-muted-foreground"
                  placeholder="Search..."
                  value={search}
                  onChange={(e) => {
                    setSearch(e.target.value);
                    setHighlightedIndex(-1);
                  }}
                  aria-label="Search options"
                />
              </div>
            )}
            <div className="max-h-60 overflow-y-auto p-1">
              {filteredOptions.length === 0 ? (
                <div className="py-2 text-center text-sm text-muted-foreground">
                  No options found
                </div>
              ) : (
                filteredOptions.map((option, index) => {
                  const isSelected = isMultiple
                    ? (value as T[]).includes(option.value)
                    : value === option.value;
                  const isHighlighted = highlightedIndex === index;

                  return (
                    <button
                      key={String(option.value)}
                      ref={(el) => {
                        optionRefs.current[index] = el;
                      }}
                      type="button"
                      role="option"
                      aria-selected={isSelected}
                      disabled={option.disabled}
                      tabIndex={isHighlighted ? 0 : -1}
                      className={cn(
                        "relative flex w-full cursor-pointer select-none items-center rounded-sm py-1.5 pl-8 pr-2 text-sm outline-none",
                        "hover:bg-accent hover:text-accent-foreground",
                        "focus:bg-accent focus:text-accent-foreground",
                        "disabled:pointer-events-none disabled:opacity-50",
                        isSelected && "bg-accent",
                        isHighlighted && "bg-accent text-accent-foreground"
                      )}
                      onClick={() => handleSelect(option)}
                      onMouseEnter={() => setHighlightedIndex(index)}
                    >
                      {isSelected && (
                        <Check className="absolute left-2 h-4 w-4" aria-hidden="true" />
                      )}
                      {option.label}
                    </button>
                  );
                })
              )}
            </div>
          </div>,
          document.body
        )}
    </>
  );
}
