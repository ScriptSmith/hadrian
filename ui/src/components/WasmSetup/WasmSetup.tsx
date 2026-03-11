import { useState, useCallback, useEffect, useId } from "react";
import {
  ArrowRight,
  ArrowLeft,
  CheckCircle2,
  XCircle,
  Loader2,
  WandSparkles,
  Plus,
  Trash2,
  ExternalLink,
  Server,
} from "lucide-react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  meProvidersCreateMutation,
  meProvidersListQueryKey,
  apiV1ModelsQueryKey,
} from "@/api/generated/@tanstack/react-query.gen";
import { meProvidersTestCredentials } from "@/api/generated/sdk.gen";
import type { ConnectivityTestResponse, DynamicProviderResponse } from "@/api/generated/types.gen";
import {
  Modal,
  ModalHeader,
  ModalTitle,
  ModalDescription,
  ModalContent,
  ModalFooter,
} from "@/components/Modal/Modal";
import { Button } from "@/components/Button/Button";
import { Input } from "@/components/Input/Input";
import { FormField } from "@/components/FormField/FormField";
import { HadrianIcon } from "@/components/HadrianIcon/HadrianIcon";
import { startOpenRouterOAuth } from "./openrouter-oauth";

interface ProviderTemplate {
  id: string;
  label: string;
  providerType: string;
  defaultBaseUrl: string;
  placeholder: string;
  description: string;
  docsUrl: string;
}

const PROVIDER_TEMPLATES: ProviderTemplate[] = [
  {
    id: "openai",
    label: "OpenAI Compatible",
    providerType: "open_ai",
    defaultBaseUrl: "https://api.openai.com/v1",
    placeholder: "sk-...",
    description: "OpenAI, OpenRouter, Ollama, and other compatible APIs",
    docsUrl: "https://platform.openai.com/api-keys",
  },
  {
    id: "anthropic",
    label: "Anthropic",
    providerType: "anthropic",
    defaultBaseUrl: "https://api.anthropic.com",
    placeholder: "sk-ant-...",
    description: "Claude Opus, Sonnet, and Haiku",
    docsUrl: "https://console.anthropic.com/settings/keys",
  },
];

type Step = "welcome" | "providers" | "done";

interface ProviderEntry {
  /** Unique key for React rendering and state tracking. */
  key: string;
  template: ProviderTemplate;
  apiKey: string;
  baseUrl: string;
  name: string;
  testResult: ConnectivityTestResponse | null;
  isTesting: boolean;
  isSaving: boolean;
  saved: boolean;
  error: string | null;
}

let entryCounter = 0;

function createEntry(template: ProviderTemplate, index: number): ProviderEntry {
  entryCounter++;
  return {
    key: `${template.id}-${entryCounter}`,
    template,
    apiKey: "",
    baseUrl: template.defaultBaseUrl,
    name: index === 0 ? template.id : `${template.id}-${index + 1}`,
    testResult: null,
    isTesting: false,
    isSaving: false,
    saved: false,
    error: null,
  };
}

function initialEntries(): ProviderEntry[] {
  return PROVIDER_TEMPLATES.map((t) => createEntry(t, 0));
}

export function WasmSetup({
  open,
  onComplete,
  oauthProviderName,
  oauthError,
  existingProviders,
  ollamaDetected,
  ollamaConnecting,
  ollamaConnected,
  onOllamaConnect,
}: {
  open: boolean;
  onComplete: () => void;
  oauthProviderName?: string | null;
  oauthError?: string | null;
  existingProviders?: DynamicProviderResponse[];
  ollamaDetected?: boolean;
  ollamaConnecting?: boolean;
  ollamaConnected?: boolean;
  onOllamaConnect?: () => void;
}) {
  const [step, setStep] = useState<Step>("welcome");
  const [entries, setEntries] = useState<ProviderEntry[]>(initialEntries);

  // Reset to welcome when the wizard is re-opened
  useEffect(() => {
    if (open) setStep("welcome");
  }, [open]);

  const [oauthLoading, setOauthLoading] = useState(false);
  const queryClient = useQueryClient();

  // Detect existing OpenRouter provider from the database
  const hasExistingOpenRouter =
    !!oauthProviderName ||
    existingProviders?.some((p) => p.base_url.includes("openrouter.ai")) === true;

  const hasExistingOllama =
    !!ollamaConnected ||
    existingProviders?.some((p) => p.base_url.includes("localhost:11434")) === true;

  const createMutation = useMutation({
    ...meProvidersCreateMutation(),
  });

  const updateEntry = useCallback((key: string, update: Partial<ProviderEntry>) => {
    setEntries((prev) => prev.map((e) => (e.key === key ? { ...e, ...update } : e)));
  }, []);

  const addEntry = useCallback((template: ProviderTemplate) => {
    setEntries((prev) => {
      const count = prev.filter((e) => e.template.id === template.id).length;
      return [...prev, createEntry(template, count)];
    });
  }, []);

  const removeEntry = useCallback((key: string) => {
    setEntries((prev) => prev.filter((e) => e.key !== key));
  }, []);

  const handleTest = useCallback(
    async (key: string) => {
      const entry = entries.find((e) => e.key === key);
      if (!entry) return;

      updateEntry(key, { isTesting: true, testResult: null, error: null });

      try {
        const { data } = await meProvidersTestCredentials({
          body: {
            name: entry.name,
            provider_type: entry.template.providerType,
            base_url: entry.baseUrl,
            api_key: entry.apiKey,
          },
        });
        updateEntry(key, { isTesting: false, testResult: data ?? null });
      } catch (err) {
        updateEntry(key, {
          isTesting: false,
          testResult: { status: "error", message: String(err) },
        });
      }
    },
    [entries, updateEntry]
  );

  const handleSave = useCallback(
    async (key: string) => {
      const entry = entries.find((e) => e.key === key);
      if (!entry) return;

      updateEntry(key, { isSaving: true, error: null });

      try {
        await createMutation.mutateAsync({
          body: {
            name: entry.name,
            provider_type: entry.template.providerType,
            base_url: entry.baseUrl,
            api_key: entry.apiKey,
          },
        });
        queryClient.invalidateQueries({ queryKey: meProvidersListQueryKey() });
        queryClient.invalidateQueries({ queryKey: apiV1ModelsQueryKey() });
        updateEntry(key, { isSaving: false, saved: true });
      } catch (err) {
        updateEntry(key, { isSaving: false, error: String(err) });
      }
    },
    [entries, createMutation, queryClient, updateEntry]
  );

  const handleOpenRouterOAuth = useCallback(async () => {
    setOauthLoading(true);
    try {
      await startOpenRouterOAuth();
    } catch {
      setOauthLoading(false);
    }
  }, []);

  const savedCount =
    entries.filter((e) => e.saved).length +
    (hasExistingOpenRouter ? 1 : 0) +
    (hasExistingOllama ? 1 : 0);
  const hasAnySaved = savedCount > 0;

  return (
    <Modal open={open} onClose={onComplete} className="max-w-lg">
      {step === "welcome" && (
        <WelcomeStep
          onNext={() => setStep("providers")}
          onReady={() => setStep("done")}
          onSkip={onComplete}
          onOpenRouterOAuth={handleOpenRouterOAuth}
          oauthLoading={oauthLoading}
          hasExistingOpenRouter={hasExistingOpenRouter}
          ollamaDetected={ollamaDetected}
          ollamaConnecting={ollamaConnecting}
          hasExistingOllama={hasExistingOllama}
          onOllamaConnect={onOllamaConnect}
        />
      )}
      {step === "providers" && (
        <ProvidersStep
          entries={entries}
          onUpdate={updateEntry}
          onAdd={addEntry}
          onRemove={removeEntry}
          onTest={handleTest}
          onSave={handleSave}
          onBack={() => setStep("welcome")}
          onNext={() => setStep("done")}
          onSkip={onComplete}
          hasAnySaved={hasAnySaved}
          hasExistingOpenRouter={hasExistingOpenRouter}
          oauthError={oauthError ?? null}
          onOpenRouterOAuth={handleOpenRouterOAuth}
          oauthLoading={oauthLoading}
          ollamaDetected={ollamaDetected}
          ollamaConnecting={ollamaConnecting}
          hasExistingOllama={hasExistingOllama}
          onOllamaConnect={onOllamaConnect}
        />
      )}
      {step === "done" && <DoneStep savedCount={savedCount} onComplete={onComplete} />}
    </Modal>
  );
}

function WelcomeStep({
  onNext,
  onReady,
  onSkip,
  onOpenRouterOAuth,
  oauthLoading,
  hasExistingOpenRouter,
  ollamaDetected,
  ollamaConnecting,
  hasExistingOllama,
  onOllamaConnect,
}: {
  onNext: () => void;
  onReady: () => void;
  onSkip: () => void;
  onOpenRouterOAuth: () => void;
  oauthLoading: boolean;
  hasExistingOpenRouter: boolean;
  ollamaDetected?: boolean;
  ollamaConnecting?: boolean;
  hasExistingOllama: boolean;
  onOllamaConnect?: () => void;
}) {
  const hasProvider = hasExistingOpenRouter || hasExistingOllama;
  return (
    <>
      <ModalHeader>
        <div className="flex items-center gap-3">
          <HadrianIcon size={36} className="text-foreground shrink-0" />
          <ModalTitle>Welcome to Hadrian</ModalTitle>
        </div>
      </ModalHeader>
      <ModalContent>
        <p className="text-sm text-muted-foreground mb-4">
          Hadrian is a free, open-source AI gateway that lets you chat with multiple models side by
          side. This is the browser edition: the gateway runs entirely in your browser.
        </p>

        <h3 className="text-base font-semibold">Connect your providers</h3>

        {hasExistingOpenRouter ? (
          <div className="mt-4 rounded-lg border border-border bg-muted/30 p-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium">OpenRouter</p>
                <p className="text-xs text-muted-foreground">https://openrouter.ai/api/v1</p>
              </div>
              <div className="flex items-center gap-1.5 text-sm text-green-700 dark:text-green-400">
                <CheckCircle2 className="h-4 w-4" />
                Connected
              </div>
            </div>
          </div>
        ) : (
          <div className="mt-4 rounded-lg border border-violet-200 bg-violet-50 p-4 dark:border-violet-500/20 dark:bg-violet-500/5">
            <p className="text-sm font-medium mb-1">OpenRouter</p>
            <p className="text-xs text-muted-foreground mb-3">
              Sign in to access 200+ models. No manual API key entry required.
            </p>
            <Button
              onClick={onOpenRouterOAuth}
              disabled={oauthLoading}
              className="w-full bg-violet-600 text-white hover:bg-violet-700 dark:bg-violet-600 dark:hover:bg-violet-500"
            >
              {oauthLoading ? (
                <Loader2 className="mr-1.5 h-4 w-4 animate-spin" />
              ) : (
                <ExternalLink className="mr-1.5 h-4 w-4" />
              )}
              Connect with OpenRouter
            </Button>
          </div>
        )}

        {hasExistingOllama ? (
          <div className="mt-3 rounded-lg border border-border bg-muted/30 p-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium">Ollama</p>
                <p className="text-xs text-muted-foreground">http://localhost:11434/v1</p>
              </div>
              <div className="flex items-center gap-1.5 text-sm text-green-700 dark:text-green-400">
                <CheckCircle2 className="h-4 w-4" />
                Connected
              </div>
            </div>
          </div>
        ) : ollamaDetected ? (
          <div className="mt-3 rounded-lg border border-emerald-200 bg-emerald-50 p-4 dark:border-emerald-500/20 dark:bg-emerald-500/5">
            <p className="text-sm font-medium mb-1">Ollama</p>
            <p className="text-xs text-muted-foreground mb-3">
              Found Ollama running at localhost:11434. Requires CORS to be enabled for this domain
              &mdash;{" "}
              <a
                href="https://hadriangateway.com/docs/browser#enable-cors"
                target="_blank"
                rel="noopener noreferrer"
                className="text-primary underline"
              >
                see setup guide
                <ExternalLink className="ml-0.5 inline h-3 w-3" />
              </a>
              .
            </p>
            <Button
              onClick={onOllamaConnect}
              disabled={ollamaConnecting}
              className="w-full bg-emerald-700 text-white hover:bg-emerald-800 dark:bg-emerald-600 dark:hover:bg-emerald-500"
            >
              {ollamaConnecting ? (
                <Loader2 className="mr-1.5 h-4 w-4 animate-spin" />
              ) : (
                <Server className="mr-1.5 h-4 w-4" />
              )}
              Connect Ollama
            </Button>
            <p className="mt-2 text-xs text-muted-foreground">
              Only models you&apos;ve pulled will appear. Try{" "}
              <code className="rounded bg-muted px-1 py-0.5 font-mono text-[11px]">
                ollama pull llama3.2
              </code>{" "}
              or{" "}
              <code className="rounded bg-muted px-1 py-0.5 font-mono text-[11px]">
                ollama pull deepseek-r1
              </code>{" "}
              to get started.
            </p>
          </div>
        ) : (
          <div className="mt-3 rounded-lg border border-border bg-muted/30 p-4">
            <div className="flex items-center justify-between gap-3">
              <div className="min-w-0">
                <p className="text-sm font-medium">Ollama</p>
                <p className="text-xs text-muted-foreground">Not detected at localhost:11434</p>
              </div>
              <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
                <XCircle className="h-3.5 w-3.5" />
                Not found
              </div>
            </div>
            <p className="mt-2 text-xs text-muted-foreground">
              Use{" "}
              <a
                href="https://ollama.com"
                target="_blank"
                rel="noopener noreferrer"
                className="text-primary underline"
              >
                Ollama
                <ExternalLink className="ml-0.5 inline h-3 w-3" />
              </a>{" "}
              to run local models for free. If Ollama is already running, it may need{" "}
              <a
                href="https://hadriangateway.com/docs/browser#enable-cors"
                target="_blank"
                rel="noopener noreferrer"
                className="text-primary underline"
              >
                CORS enabled
                <ExternalLink className="ml-0.5 inline h-3 w-3" />
              </a>{" "}
              to be detected.
            </p>
          </div>
        )}

        <p className="text-sm text-muted-foreground mt-4">
          {hasProvider
            ? "You can also add API keys from OpenAI, Anthropic, or other providers."
            : "Or add your own API keys from OpenAI, Anthropic, or other providers."}
        </p>
        <p className="text-xs text-muted-foreground mt-4">
          For the server version with teams, SSO, guardrails, and more, see{" "}
          <a
            href="https://hadriangateway.com"
            target="_blank"
            rel="noopener noreferrer"
            className="text-primary underline"
          >
            hadriangateway.com
          </a>
          .
        </p>
      </ModalContent>
      <ModalFooter>
        {hasProvider ? (
          <>
            <Button variant="outline" onClick={onNext}>
              Add more providers
              <ArrowRight className="ml-1.5 h-4 w-4" />
            </Button>
            <Button onClick={onReady}>
              Next
              <ArrowRight className="ml-1.5 h-4 w-4" />
            </Button>
          </>
        ) : (
          <>
            <Button variant="ghost" onClick={onSkip}>
              Skip for now
            </Button>
            <Button variant="outline" onClick={onNext}>
              Add API keys manually
              <ArrowRight className="ml-1.5 h-4 w-4" />
            </Button>
          </>
        )}
      </ModalFooter>
    </>
  );
}

function ProvidersStep({
  entries,
  onUpdate,
  onAdd,
  onRemove,
  onTest,
  onSave,
  onBack,
  onNext,
  onSkip,
  hasAnySaved,
  hasExistingOpenRouter,
  oauthError,
  onOpenRouterOAuth,
  oauthLoading,
  ollamaDetected,
  ollamaConnecting,
  hasExistingOllama,
  onOllamaConnect,
}: {
  entries: ProviderEntry[];
  onUpdate: (key: string, update: Partial<ProviderEntry>) => void;
  onAdd: (template: ProviderTemplate) => void;
  onRemove: (key: string) => void;
  onTest: (key: string) => void;
  onSave: (key: string) => void;
  onBack: () => void;
  onNext: () => void;
  onSkip: () => void;
  hasAnySaved: boolean;
  hasExistingOpenRouter: boolean;
  oauthError: string | null;
  onOpenRouterOAuth: () => void;
  oauthLoading: boolean;
  ollamaDetected?: boolean;
  ollamaConnecting?: boolean;
  hasExistingOllama: boolean;
  onOllamaConnect?: () => void;
}) {
  return (
    <>
      <ModalHeader>
        <ModalTitle>Connect your providers</ModalTitle>
        <ModalDescription>Add at least one API key to start chatting</ModalDescription>
      </ModalHeader>
      <ModalContent>
        {/* OpenRouter OAuth section */}
        {hasExistingOpenRouter ? (
          <div className="mb-4 rounded-lg border border-border bg-muted/30 p-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium">OpenRouter</p>
                <p className="text-xs text-muted-foreground">https://openrouter.ai/api/v1</p>
              </div>
              <div className="flex items-center gap-1.5 text-sm text-green-700 dark:text-green-400">
                <CheckCircle2 className="h-4 w-4" />
                Connected
              </div>
            </div>
          </div>
        ) : (
          <div className="mb-4 rounded-lg border border-violet-200 bg-violet-50/50 p-3 dark:border-violet-500/20 dark:bg-violet-500/5">
            <div className="flex items-center justify-between gap-3">
              <div className="min-w-0">
                <p className="text-sm font-medium">OpenRouter</p>
                <p className="text-xs text-muted-foreground">200+ models, one click</p>
              </div>
              <Button
                size="sm"
                onClick={onOpenRouterOAuth}
                disabled={oauthLoading}
                className="shrink-0 bg-violet-600 text-white hover:bg-violet-700 dark:bg-violet-600 dark:hover:bg-violet-500"
              >
                {oauthLoading ? (
                  <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
                ) : (
                  <ExternalLink className="mr-1.5 h-3.5 w-3.5" />
                )}
                Connect
              </Button>
            </div>
          </div>
        )}

        {oauthError && (
          <div className="mb-4 flex items-start gap-1.5 text-xs text-destructive">
            <XCircle className="mt-0.5 h-3 w-3 shrink-0" />
            <span>OpenRouter connection failed: {oauthError}</span>
          </div>
        )}

        {/* Ollama section */}
        {hasExistingOllama ? (
          <div className="mb-4 rounded-lg border border-border bg-muted/30 p-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium">Ollama</p>
                <p className="text-xs text-muted-foreground">http://localhost:11434/v1</p>
              </div>
              <div className="flex items-center gap-1.5 text-sm text-green-700 dark:text-green-400">
                <CheckCircle2 className="h-4 w-4" />
                Connected
              </div>
            </div>
          </div>
        ) : ollamaDetected ? (
          <div className="mb-4 rounded-lg border border-emerald-200 bg-emerald-50/50 p-3 dark:border-emerald-500/20 dark:bg-emerald-500/5">
            <div className="flex items-center justify-between gap-3">
              <div className="min-w-0">
                <p className="text-sm font-medium">Ollama</p>
                <p className="text-xs text-muted-foreground">
                  Local models detected &mdash;{" "}
                  <a
                    href="https://hadriangateway.com/docs/browser#enable-cors"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-primary underline"
                  >
                    CORS required
                  </a>
                </p>
              </div>
              <Button
                size="sm"
                onClick={onOllamaConnect}
                disabled={ollamaConnecting}
                className="shrink-0 bg-emerald-700 text-white hover:bg-emerald-800 dark:bg-emerald-600 dark:hover:bg-emerald-500"
              >
                {ollamaConnecting ? (
                  <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Server className="mr-1.5 h-3.5 w-3.5" />
                )}
                Connect
              </Button>
            </div>
          </div>
        ) : null}

        <div className="space-y-5">
          {entries.map((entry) => (
            <ProviderKeyEntry
              key={entry.key}
              entry={entry}
              canRemove={entries.filter((e) => e.template.id === entry.template.id).length > 1}
              onUpdate={(update) => onUpdate(entry.key, update)}
              onRemove={() => onRemove(entry.key)}
              onTest={() => onTest(entry.key)}
              onSave={() => onSave(entry.key)}
            />
          ))}
        </div>
        <div className="mt-4 flex flex-wrap gap-2">
          {PROVIDER_TEMPLATES.map((t) => (
            <Button key={t.id} variant="ghost" size="sm" onClick={() => onAdd(t)}>
              <Plus className="mr-1.5 h-3 w-3" />
              Add {t.label}
            </Button>
          ))}
        </div>
      </ModalContent>
      <ModalFooter>
        <Button variant="ghost" onClick={onBack}>
          <ArrowLeft className="mr-1.5 h-4 w-4" />
          Back
        </Button>
        <div className="flex gap-2">
          {!hasAnySaved && (
            <Button variant="ghost" onClick={onSkip}>
              Skip
            </Button>
          )}
          {hasAnySaved && (
            <Button onClick={onNext}>
              Continue
              <ArrowRight className="ml-1.5 h-4 w-4" />
            </Button>
          )}
        </div>
      </ModalFooter>
    </>
  );
}

function ProviderKeyEntry({
  entry,
  canRemove,
  onUpdate,
  onRemove,
  onTest,
  onSave,
}: {
  entry: ProviderEntry;
  canRemove: boolean;
  onUpdate: (update: Partial<ProviderEntry>) => void;
  onRemove: () => void;
  onTest: () => void;
  onSave: () => void;
}) {
  const id = useId();
  const { template } = entry;

  if (entry.saved) {
    return (
      <div className="rounded-lg border border-border bg-muted/30 p-4">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm font-medium">{entry.name}</p>
            <p className="text-xs text-muted-foreground">{entry.baseUrl}</p>
          </div>
          <div className="flex items-center gap-1.5 text-sm text-green-700 dark:text-green-400">
            <CheckCircle2 className="h-4 w-4" />
            Connected
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-border p-4">
      <div className="mb-3">
        <div className="flex items-center justify-between">
          <p className="text-sm font-medium">{template.label}</p>
          <div className="flex items-center gap-3">
            <a
              href={template.docsUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="text-xs text-primary hover:underline"
            >
              Get API key
            </a>
            {canRemove && (
              <button
                type="button"
                onClick={onRemove}
                className="text-muted-foreground hover:text-destructive"
                aria-label={`Remove ${template.label} provider`}
              >
                <Trash2 className="h-3.5 w-3.5" />
              </button>
            )}
          </div>
        </div>
        <p className="text-xs text-muted-foreground">{template.description}</p>
      </div>

      <div className="space-y-2">
        <FormField label="API Key (optional for local models)" htmlFor={`${id}-key`}>
          <Input
            id={`${id}-key`}
            type="password"
            placeholder={template.placeholder}
            value={entry.apiKey}
            onChange={(e) => onUpdate({ apiKey: e.target.value })}
            disabled={entry.isSaving}
          />
        </FormField>

        <FormField label="Base URL" htmlFor={`${id}-url`}>
          <Input
            id={`${id}-url`}
            type="url"
            placeholder={template.defaultBaseUrl}
            value={entry.baseUrl}
            onChange={(e) => onUpdate({ baseUrl: e.target.value })}
            disabled={entry.isSaving}
          />
        </FormField>
      </div>

      <div className="mt-3 flex gap-2">
        <Button
          variant="outline"
          size="sm"
          onClick={onTest}
          disabled={entry.isTesting || entry.isSaving}
        >
          {entry.isTesting ? <Loader2 className="h-4 w-4 animate-spin" /> : "Test"}
        </Button>
        <Button size="sm" onClick={onSave} disabled={entry.isSaving || entry.isTesting}>
          {entry.isSaving ? <Loader2 className="h-4 w-4 animate-spin" /> : "Save"}
        </Button>
      </div>

      {entry.testResult && (
        <div className="mt-2">
          {entry.testResult.status === "ok" ? (
            <div className="flex items-center gap-1.5 text-xs text-green-700 dark:text-green-400">
              <CheckCircle2 className="h-3 w-3" />
              {entry.testResult.message}
              {entry.testResult.latency_ms != null && (
                <span className="text-muted-foreground">({entry.testResult.latency_ms}ms)</span>
              )}
            </div>
          ) : (
            <div className="flex items-center gap-1.5 text-xs text-destructive">
              <XCircle className="h-3 w-3" />
              {entry.testResult.message}
            </div>
          )}
        </div>
      )}

      {entry.error && <p className="mt-2 text-xs text-destructive">{entry.error}</p>}
    </div>
  );
}

function DoneStep({ savedCount, onComplete }: { savedCount: number; onComplete: () => void }) {
  return (
    <>
      <ModalHeader>
        <ModalTitle>Setup complete</ModalTitle>
        <ModalDescription>
          {savedCount} provider{savedCount !== 1 ? "s" : ""} connected
        </ModalDescription>
      </ModalHeader>
      <ModalContent>
        <div className="flex flex-col items-center py-4 text-center">
          <div className="mb-4 flex h-12 w-12 items-center justify-center rounded-full bg-green-500/10">
            <WandSparkles className="h-6 w-6 text-green-700 dark:text-green-400" />
          </div>
          <p className="text-sm text-muted-foreground max-w-sm">
            Manage providers from the <strong>Providers</strong> page in the sidebar, or re-run this
            wizard using the <WandSparkles className="inline h-3.5 w-3.5" /> icon in the top right.
          </p>
        </div>
      </ModalContent>
      <ModalFooter>
        <Button onClick={onComplete}>
          Start chatting
          <ArrowRight className="ml-1.5 h-4 w-4" />
        </Button>
      </ModalFooter>
    </>
  );
}
