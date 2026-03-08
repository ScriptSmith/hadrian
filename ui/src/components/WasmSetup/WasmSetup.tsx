import { useState, useCallback, useId } from "react";
import {
  Globe,
  Lock,
  ArrowRight,
  ArrowLeft,
  CheckCircle2,
  XCircle,
  Loader2,
  Sparkles,
  Plus,
  Trash2,
} from "lucide-react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  meProvidersCreateMutation,
  meProvidersListQueryKey,
  apiV1ModelsQueryKey,
} from "@/api/generated/@tanstack/react-query.gen";
import { meProvidersTestCredentials } from "@/api/generated/sdk.gen";
import type { ConnectivityTestResponse } from "@/api/generated/types.gen";
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

export function WasmSetup({ open, onComplete }: { open: boolean; onComplete: () => void }) {
  const [step, setStep] = useState<Step>("welcome");
  const [entries, setEntries] = useState<ProviderEntry[]>(initialEntries);
  const queryClient = useQueryClient();

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

  const savedCount = entries.filter((e) => e.saved).length;
  const hasAnySaved = savedCount > 0;

  return (
    <Modal open={open} onClose={onComplete} className="max-w-lg">
      {step === "welcome" && (
        <WelcomeStep onNext={() => setStep("providers")} onSkip={onComplete} />
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
        />
      )}
      {step === "done" && <DoneStep savedCount={savedCount} onComplete={onComplete} />}
    </Modal>
  );
}

function WelcomeStep({ onNext, onSkip }: { onNext: () => void; onSkip: () => void }) {
  return (
    <>
      <ModalHeader>
        <ModalTitle>Welcome to Hadrian</ModalTitle>
        <ModalDescription>Browser-powered AI gateway</ModalDescription>
      </ModalHeader>
      <ModalContent>
        <p className="text-sm text-muted-foreground mb-6">
          This is the standalone browser edition of Hadrian. The AI gateway runs locally in your
          browser — there is no Hadrian server.
        </p>
        <div className="space-y-4">
          <div className="flex gap-3">
            <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary/10">
              <Globe className="h-4 w-4 text-primary" />
            </div>
            <div>
              <p className="text-sm font-medium">No backend server</p>
              <p className="text-xs text-muted-foreground">
                The gateway and all your data live in your browser's local storage. Nothing is sent
                to Hadrian — requests go directly from your browser to the AI providers you
                configure.
              </p>
            </div>
          </div>
          <div className="flex gap-3">
            <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary/10">
              <Lock className="h-4 w-4 text-primary" />
            </div>
            <div>
              <p className="text-sm font-medium">Direct to provider</p>
              <p className="text-xs text-muted-foreground">
                API keys and conversations are stored locally. When you send a message, it goes
                directly to the provider (OpenAI, Anthropic) — not through any intermediary.
              </p>
            </div>
          </div>
        </div>
        <p className="text-sm text-muted-foreground mt-6">
          To get started, you'll need an API key from at least one provider.
        </p>
      </ModalContent>
      <ModalFooter>
        <Button variant="ghost" onClick={onSkip}>
          Skip for now
        </Button>
        <Button onClick={onNext}>
          Add provider keys
          <ArrowRight className="ml-1.5 h-4 w-4" />
        </Button>
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
}) {
  return (
    <>
      <ModalHeader>
        <ModalTitle>Connect your providers</ModalTitle>
        <ModalDescription>Add at least one API key to start chatting</ModalDescription>
      </ModalHeader>
      <ModalContent>
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
          <div className="flex items-center gap-1.5 text-sm text-green-600 dark:text-green-400">
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
            <div className="flex items-center gap-1.5 text-xs text-green-600 dark:text-green-400">
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
        <ModalTitle>You're all set</ModalTitle>
        <ModalDescription>
          {savedCount} provider{savedCount !== 1 ? "s" : ""} connected
        </ModalDescription>
      </ModalHeader>
      <ModalContent>
        <div className="flex flex-col items-center py-4 text-center">
          <div className="mb-4 flex h-12 w-12 items-center justify-center rounded-full bg-green-500/10">
            <Sparkles className="h-6 w-6 text-green-600 dark:text-green-400" />
          </div>
          <p className="text-sm text-muted-foreground max-w-sm">
            You can manage your providers anytime from the <strong>Providers</strong> page in the
            sidebar, or re-run this wizard from the user menu.
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
