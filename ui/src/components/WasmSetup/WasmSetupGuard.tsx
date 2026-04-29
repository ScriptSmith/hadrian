import {
  createContext,
  useContext,
  useState,
  useCallback,
  useEffect,
  useRef,
  type ReactNode,
} from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  meProvidersListOptions,
  meProvidersCreateMutation,
  meProvidersListQueryKey,
  apiV1ModelsQueryKey,
} from "@/api/generated/@tanstack/react-query.gen";
import { WasmSetup } from "./WasmSetup";
import type { BrowserAiState } from "./BrowserAiCard";
import { formatApiError } from "@/utils/formatApiError";
import {
  getOpenRouterCallbackCode,
  clearCallbackCode,
  exchangeCodeForKey,
} from "./openrouter-oauth";
import {
  getAvailability,
  getLanguageModel,
  installBrowserAiBridge,
  isLanguageModelSupported,
} from "@/services/browser-ai";

const IS_WASM = import.meta.env.VITE_WASM_MODE === "true";
const DISMISSED_KEY = "hadrian-wasm-setup-dismissed";

interface WasmSetupContextValue {
  /** True when running in WASM mode. */
  isWasm: boolean;
  /** Open the setup wizard. */
  openSetupWizard: () => void;
  /** Name of provider just connected via OAuth, if any. */
  oauthProviderName: string | null;
  /** Clear the OAuth success state. */
  clearOAuthSuccess: () => void;
}

const WasmSetupContext = createContext<WasmSetupContextValue>({
  isWasm: false,
  openSetupWizard: () => {},
  oauthProviderName: null,
  clearOAuthSuccess: () => {},
});

/** Access WASM setup state. Only meaningful when `isWasm` is true. */
export function useWasmSetup() {
  return useContext(WasmSetupContext);
}

/**
 * In WASM mode, shows the onboarding wizard if no providers are configured
 * and the user hasn't dismissed it before. In server mode, renders children directly.
 *
 * Also handles OpenRouter OAuth callbacks: if the URL contains a `code` param,
 * exchanges it for an API key and saves it as a dynamic provider.
 */
export function WasmSetupGuard({ children }: { children: ReactNode }) {
  const [dismissed, setDismissed] = useState(() => localStorage.getItem(DISMISSED_KEY) === "true");
  const [manualOpen, setManualOpen] = useState(false);
  const [oauthProviderName, setOAuthProviderName] = useState<string | null>(null);
  const [oauthError, setOAuthError] = useState<string | null>(null);
  const oauthHandled = useRef(false);
  const [ollamaDetected, setOllamaDetected] = useState(false);
  const [ollamaConnecting, setOllamaConnecting] = useState(false);
  const [ollamaConnected, setOllamaConnected] = useState(false);
  const [browserAi, setBrowserAi] = useState<BrowserAiState>(() => ({
    supported: IS_WASM ? isLanguageModelSupported() : false,
    availability: "unavailable",
    downloadProgress: null,
    downloading: false,
    error: null,
  }));
  const queryClient = useQueryClient();

  const createProvider = useMutation({ ...meProvidersCreateMutation() });

  const { data, isLoading } = useQuery({
    ...meProvidersListOptions(),
    enabled: IS_WASM,
  });

  // Detect local Ollama instance
  useEffect(() => {
    if (!IS_WASM) return;
    const controller = new AbortController();
    fetch("http://localhost:11434/v1/models", { signal: controller.signal })
      .then((res) => {
        if (res.ok) setOllamaDetected(true);
      })
      .catch(() => {});
    return () => controller.abort();
  }, []);

  // Install the LanguageModel bridge so the WASM service worker can reach
  // the on-device Prompt API (only exposed in window scope), and surface the
  // current availability state for the UI.
  useEffect(() => {
    if (!IS_WASM) return;
    if (!isLanguageModelSupported()) return;
    const uninstall = installBrowserAiBridge();
    let cancelled = false;
    getAvailability().then((state) => {
      if (cancelled) return;
      setBrowserAi((prev) => ({ ...prev, availability: state }));
    });
    return () => {
      cancelled = true;
      uninstall();
    };
  }, []);

  const handleBrowserAiDownload = useCallback(async () => {
    const lm = getLanguageModel();
    if (!lm) return;
    setBrowserAi((prev) => ({
      ...prev,
      downloading: true,
      downloadProgress: 0,
      availability: "downloading",
      error: null,
    }));
    try {
      const session = await lm.create({
        monitor(m) {
          m.addEventListener("downloadprogress", (event) => {
            setBrowserAi((prev) => ({ ...prev, downloadProgress: event.loaded }));
          });
        },
      });
      session.destroy();
      const next = await getAvailability();
      setBrowserAi((prev) => ({
        ...prev,
        availability: next,
        downloading: false,
        downloadProgress: null,
      }));
      queryClient.invalidateQueries({ queryKey: apiV1ModelsQueryKey() });
    } catch (err) {
      setBrowserAi((prev) => ({
        ...prev,
        downloading: false,
        downloadProgress: null,
        availability: "downloadable",
        error: formatApiError(err),
      }));
    }
  }, [queryClient]);

  const handleOllamaConnect = useCallback(async () => {
    setOllamaConnecting(true);
    try {
      await createProvider.mutateAsync({
        body: {
          name: "ollama",
          provider_type: "open_ai",
          base_url: "http://localhost:11434/v1",
          api_key: "ollama",
        },
      });
      queryClient.invalidateQueries({ queryKey: meProvidersListQueryKey() });
      queryClient.invalidateQueries({ queryKey: apiV1ModelsQueryKey() });
      setOllamaConnected(true);
      setManualOpen(true);
    } catch (err) {
      console.error("Ollama connect failed:", err);
    } finally {
      setOllamaConnecting(false);
    }
  }, [createProvider, queryClient]);

  // Handle OpenRouter OAuth callback
  useEffect(() => {
    if (!IS_WASM || oauthHandled.current) return;
    const code = getOpenRouterCallbackCode();
    if (!code) return;
    oauthHandled.current = true;
    clearCallbackCode();

    (async () => {
      try {
        const apiKey = await exchangeCodeForKey(code);
        await createProvider.mutateAsync({
          body: {
            name: "openrouter",
            provider_type: "open_ai",
            base_url: "https://openrouter.ai/api/v1",
            api_key: apiKey,
          },
        });
        queryClient.invalidateQueries({ queryKey: meProvidersListQueryKey() });
        queryClient.invalidateQueries({ queryKey: apiV1ModelsQueryKey() });
        setOAuthProviderName("openrouter");
        setManualOpen(true);
      } catch (err) {
        console.error("OpenRouter OAuth failed:", err);
        setOAuthError(formatApiError(err));
        setManualOpen(true);
      }
    })();
  }, [createProvider, queryClient]);

  const openSetupWizard = useCallback(() => setManualOpen(true), []);

  const handleComplete = useCallback(() => {
    localStorage.setItem(DISMISSED_KEY, "true");
    setDismissed(true);
    setManualOpen(false);
    setOAuthProviderName(null);
    setOAuthError(null);
    setOllamaConnected(false);
  }, []);

  const clearOAuthSuccess = useCallback(() => {
    setOAuthProviderName(null);
    setOAuthError(null);
  }, []);

  const contextValue: WasmSetupContextValue = {
    isWasm: IS_WASM,
    openSetupWizard,
    oauthProviderName,
    clearOAuthSuccess,
  };

  if (!IS_WASM) {
    return <WasmSetupContext.Provider value={contextValue}>{children}</WasmSetupContext.Provider>;
  }

  // Auto-show: no providers and not previously dismissed. Browser AI counts
  // as a provider once the model is ready locally, since requests against
  // it succeed without any setup the wizard could prompt for.
  const dynamicProviderCount = data?.data?.length ?? 0;
  const browserAiCounts = browserAi.supported && browserAi.availability === "available";
  const needsOnboarding =
    !dismissed && !isLoading && dynamicProviderCount === 0 && !browserAiCounts;

  return (
    <WasmSetupContext.Provider value={contextValue}>
      {children}
      <WasmSetup
        open={needsOnboarding || manualOpen}
        onComplete={handleComplete}
        oauthProviderName={oauthProviderName}
        oauthError={oauthError}
        existingProviders={data?.data}
        ollamaDetected={ollamaDetected}
        ollamaConnecting={ollamaConnecting}
        ollamaConnected={ollamaConnected}
        onOllamaConnect={handleOllamaConnect}
        browserAi={browserAi}
        onBrowserAiDownload={handleBrowserAiDownload}
      />
    </WasmSetupContext.Provider>
  );
}
