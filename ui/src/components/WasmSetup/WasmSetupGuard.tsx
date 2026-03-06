import { createContext, useContext, useState, useCallback, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { meProvidersListOptions } from "@/api/generated/@tanstack/react-query.gen";
import { WasmSetup } from "./WasmSetup";

const IS_WASM = import.meta.env.VITE_WASM_MODE === "true";
const DISMISSED_KEY = "hadrian-wasm-setup-dismissed";

interface WasmSetupContextValue {
  /** True when running in WASM mode. */
  isWasm: boolean;
  /** Open the setup wizard. */
  openSetupWizard: () => void;
}

const WasmSetupContext = createContext<WasmSetupContextValue>({
  isWasm: false,
  openSetupWizard: () => {},
});

/** Access WASM setup state. Only meaningful when `isWasm` is true. */
export function useWasmSetup() {
  return useContext(WasmSetupContext);
}

/**
 * In WASM mode, shows the onboarding wizard if no providers are configured
 * and the user hasn't dismissed it before. In server mode, renders children directly.
 *
 * Also provides `useWasmSetup()` so other components (e.g. user menu) can re-open the wizard.
 */
export function WasmSetupGuard({ children }: { children: ReactNode }) {
  const [dismissed, setDismissed] = useState(() => localStorage.getItem(DISMISSED_KEY) === "true");
  const [manualOpen, setManualOpen] = useState(false);

  const { data, isLoading } = useQuery({
    ...meProvidersListOptions(),
    enabled: IS_WASM && !dismissed,
  });

  const openSetupWizard = useCallback(() => setManualOpen(true), []);

  const handleComplete = useCallback(() => {
    localStorage.setItem(DISMISSED_KEY, "true");
    setDismissed(true);
    setManualOpen(false);
  }, []);

  const contextValue: WasmSetupContextValue = { isWasm: IS_WASM, openSetupWizard };

  if (!IS_WASM) {
    return <WasmSetupContext.Provider value={contextValue}>{children}</WasmSetupContext.Provider>;
  }

  // Auto-show: no providers and not previously dismissed
  const needsOnboarding = !dismissed && !isLoading && (data?.data?.length ?? 0) === 0;

  return (
    <WasmSetupContext.Provider value={contextValue}>
      {children}
      <WasmSetup open={needsOnboarding || manualOpen} onComplete={handleComplete} />
    </WasmSetupContext.Provider>
  );
}
