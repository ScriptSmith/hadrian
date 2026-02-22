import { useState, createContext, useContext, useCallback, type ReactNode } from "react";
import { AlertTriangle } from "lucide-react";
import { Modal, ModalHeader, ModalContent, ModalFooter } from "@/components/Modal/Modal";
import { Button } from "@/components/Button/Button";
import { cn } from "@/utils/cn";

interface ConfirmOptions {
  title?: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  variant?: "default" | "destructive";
}

interface ConfirmDialogContextValue {
  confirm: (options: ConfirmOptions) => Promise<boolean>;
}

const ConfirmDialogContext = createContext<ConfirmDialogContextValue | null>(null);

export function useConfirm() {
  const context = useContext(ConfirmDialogContext);
  if (!context) {
    throw new Error("useConfirm must be used within a ConfirmDialogProvider");
  }
  return context.confirm;
}

interface ConfirmDialogProviderProps {
  children: ReactNode;
}

export function ConfirmDialogProvider({ children }: ConfirmDialogProviderProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [options, setOptions] = useState<ConfirmOptions | null>(null);
  const [resolveRef, setResolveRef] = useState<((value: boolean) => void) | null>(null);

  const confirm = useCallback((opts: ConfirmOptions): Promise<boolean> => {
    return new Promise((resolve) => {
      setOptions(opts);
      setResolveRef(() => resolve);
      setIsOpen(true);
    });
  }, []);

  const handleConfirm = useCallback(() => {
    setIsOpen(false);
    resolveRef?.(true);
    setResolveRef(null);
    setOptions(null);
  }, [resolveRef]);

  const handleCancel = useCallback(() => {
    setIsOpen(false);
    resolveRef?.(false);
    setResolveRef(null);
    setOptions(null);
  }, [resolveRef]);

  const isDestructive = options?.variant === "destructive";

  return (
    <ConfirmDialogContext.Provider value={{ confirm }}>
      {children}
      <Modal open={isOpen} onClose={handleCancel}>
        <ModalHeader>
          <div className="flex items-center gap-3">
            {isDestructive && (
              <div className="flex h-10 w-10 items-center justify-center rounded-full bg-destructive/10">
                <AlertTriangle className="h-5 w-5 text-destructive" />
              </div>
            )}
            <span>{options?.title || "Confirm"}</span>
          </div>
        </ModalHeader>
        <ModalContent>
          <p className={cn("text-sm text-muted-foreground", isDestructive && "ml-13")}>
            {options?.message}
          </p>
        </ModalContent>
        <ModalFooter>
          <Button type="button" variant="ghost" onClick={handleCancel}>
            {options?.cancelLabel || "Cancel"}
          </Button>
          <Button
            type="button"
            variant={isDestructive ? "danger" : "primary"}
            onClick={handleConfirm}
          >
            {options?.confirmLabel || "Confirm"}
          </Button>
        </ModalFooter>
      </Modal>
    </ConfirmDialogContext.Provider>
  );
}
