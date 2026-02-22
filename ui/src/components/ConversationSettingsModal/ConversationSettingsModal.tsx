import { useState } from "react";
import { Settings2, RotateCcw, Database, Sparkles, Save, ChevronDown, Volume2 } from "lucide-react";

import type { VectorStoreOwnerType, Prompt, Voice } from "@/api/generated/types.gen";
import { TTS_VOICES, DEFAULT_TTS_VOICE, DEFAULT_TTS_SPEED } from "@/hooks/useAudioPlayback";
import { Button } from "@/components/Button/Button";
import type { ResponseActionConfig } from "@/components/chat-types";
import { DEFAULT_ACTION_CONFIG } from "@/components/chat-types";
import {
  Modal,
  ModalClose,
  ModalContent,
  ModalFooter,
  ModalHeader,
  ModalTitle,
} from "@/components/Modal/Modal";
import {
  Dropdown,
  DropdownContent,
  DropdownItem,
  DropdownTrigger,
} from "@/components/Dropdown/Dropdown";
import { PromptFormModal } from "@/components/PromptFormModal/PromptFormModal";
import { Select, type SelectOption } from "@/components/Select/Select";
import { Slider } from "@/components/Slider/Slider";
import { Switch } from "@/components/Switch/Switch";
import { VectorStoreSelector } from "@/components/VectorStores/VectorStoreSelector";
import { useUserPrompts, type PromptWithOrg } from "@/hooks/useUserPrompts";

interface ConversationSettingsModalProps {
  open: boolean;
  onClose: () => void;
  /** Global system prompt (fallback when no per-model prompt is set) */
  systemPrompt: string;
  onSystemPromptChange: (value: string) => void;
  actionConfig?: ResponseActionConfig;
  onActionConfigChange?: (config: ResponseActionConfig) => void;
  /** Attached vector store IDs for file_search (RAG) */
  vectorStoreIds?: string[];
  /** Callback when vector store selection changes */
  onVectorStoreIdsChange?: (ids: string[]) => void;
  /** Owner type for filtering available vector stores */
  vectorStoreOwnerType?: VectorStoreOwnerType;
  /** Owner ID for filtering available vector stores */
  vectorStoreOwnerId?: string;
  /** Whether client-side RAG execution is enabled */
  clientSideRAG?: boolean;
  /** Callback when client-side RAG setting changes */
  onClientSideRAGChange?: (enabled: boolean) => void;
  /** Whether to capture raw SSE events for debugging */
  captureRawSSEEvents?: boolean;
  /** Callback when SSE capture setting changes */
  onCaptureRawSSEEventsChange?: (enabled: boolean) => void;
  /** Current TTS voice preference */
  ttsVoice?: Voice;
  /** Callback when TTS voice changes */
  onTTSVoiceChange?: (voice: Voice) => void;
  /** Current TTS playback speed preference */
  ttsSpeed?: number;
  /** Callback when TTS speed changes */
  onTTSSpeedChange?: (speed: number) => void;
}

/** Voice options for the TTS voice selector */
const VOICE_OPTIONS: SelectOption<Voice>[] = TTS_VOICES.map((voice) => ({
  value: voice,
  label: voice.charAt(0).toUpperCase() + voice.slice(1),
}));

export function ConversationSettingsModal({
  open,
  onClose,
  systemPrompt,
  onSystemPromptChange,
  actionConfig = DEFAULT_ACTION_CONFIG,
  onActionConfigChange,
  vectorStoreIds,
  onVectorStoreIdsChange,
  vectorStoreOwnerType,
  vectorStoreOwnerId,
  clientSideRAG,
  onClientSideRAGChange,
  captureRawSSEEvents,
  onCaptureRawSSEEventsChange,
  ttsVoice = DEFAULT_TTS_VOICE,
  onTTSVoiceChange,
  ttsSpeed = DEFAULT_TTS_SPEED,
  onTTSSpeedChange,
}: ConversationSettingsModalProps) {
  const [promptFormOpen, setPromptFormOpen] = useState(false);
  const [editingPrompt, setEditingPrompt] = useState<Prompt | null>(null);
  const { prompts, isLoading: promptsLoading } = useUserPrompts();

  const handleApplyPrompt = (prompt: PromptWithOrg) => {
    onSystemPromptChange(prompt.content);
  };

  const handleSaveAsTemplate = () => {
    setEditingPrompt(null);
    setPromptFormOpen(true);
  };

  const handlePromptSaved = (_prompt: Prompt) => {
    // Prompt saved successfully, modal will close automatically
  };

  const updateActionConfig = <K extends keyof ResponseActionConfig>(
    key: K,
    value: ResponseActionConfig[K]
  ) => {
    if (onActionConfigChange) {
      onActionConfigChange({ ...actionConfig, [key]: value });
    }
  };

  const hasActionConfigChanges =
    actionConfig.showSelectBest !== DEFAULT_ACTION_CONFIG.showSelectBest ||
    actionConfig.showRegenerate !== DEFAULT_ACTION_CONFIG.showRegenerate ||
    actionConfig.showCopy !== DEFAULT_ACTION_CONFIG.showCopy ||
    actionConfig.showExpand !== DEFAULT_ACTION_CONFIG.showExpand;

  const resetActionConfig = () => {
    if (onActionConfigChange) {
      onActionConfigChange(DEFAULT_ACTION_CONFIG);
    }
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      className="max-w-2xl max-h-[85vh] overflow-hidden flex flex-col"
    >
      <ModalClose onClose={onClose} />
      <ModalHeader>
        <ModalTitle className="flex items-center gap-2">
          <Settings2 className="h-5 w-5" />
          Conversation Settings
        </ModalTitle>
      </ModalHeader>

      <ModalContent className="overflow-y-auto flex-1">
        {/* System Prompt Section */}
        <div className="space-y-3 mb-6">
          <div className="flex items-start justify-between gap-2">
            <div className="flex-1">
              <h3 className="text-sm font-medium mb-1">System Prompt</h3>
              <p className="text-xs text-muted-foreground">
                Default instructions for all models. Individual models can override this via the
                settings icon on each model chip.
              </p>
            </div>
            <div className="flex items-center gap-2 shrink-0">
              {/* Template Selector */}
              <Dropdown>
                <DropdownTrigger asChild>
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-8 px-2 gap-1"
                    disabled={promptsLoading || prompts.length === 0}
                  >
                    <Sparkles className="h-3.5 w-3.5" />
                    <span className="hidden sm:inline">Templates</span>
                    <ChevronDown className="h-3 w-3" />
                  </Button>
                </DropdownTrigger>
                <DropdownContent align="end" className="w-56 max-h-64 overflow-y-auto">
                  {prompts.length === 0 ? (
                    <div className="px-2 py-3 text-sm text-muted-foreground text-center">
                      No templates available
                    </div>
                  ) : (
                    prompts.map((prompt) => (
                      <DropdownItem
                        key={prompt.id}
                        onClick={() => handleApplyPrompt(prompt)}
                        className="flex flex-col items-start gap-0.5"
                      >
                        <span className="font-medium">{prompt.name}</span>
                        {prompt.description && (
                          <span className="text-xs text-muted-foreground line-clamp-1">
                            {prompt.description}
                          </span>
                        )}
                      </DropdownItem>
                    ))
                  )}
                </DropdownContent>
              </Dropdown>

              {/* Save as Template Button */}
              <Button
                variant="outline"
                size="sm"
                className="h-8 px-2 gap-1"
                onClick={handleSaveAsTemplate}
                disabled={!systemPrompt.trim()}
                title={systemPrompt.trim() ? "Save as template" : "Enter a prompt to save"}
              >
                <Save className="h-3.5 w-3.5" />
                <span className="hidden sm:inline">Save</span>
              </Button>
            </div>
          </div>
          <textarea
            value={systemPrompt}
            onChange={(e) => onSystemPromptChange(e.target.value)}
            placeholder="Enter system instructions..."
            aria-label="System prompt"
            className="w-full min-h-[120px] rounded-md border bg-background px-3 py-2 text-sm placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 resize-y"
          />
        </div>

        {/* Knowledge Base Section */}
        {onVectorStoreIdsChange && (
          <div className="space-y-3 mb-6">
            <div>
              <h3 className="text-sm font-medium mb-1 flex items-center gap-2">
                <Database className="h-4 w-4" />
                Knowledge Base
              </h3>
              <p className="text-xs text-muted-foreground mb-2">
                Attach knowledge bases to enable retrieval-augmented generation (RAG). The model
                will search these to find relevant context when answering questions.
              </p>
            </div>
            <VectorStoreSelector
              selectedIds={vectorStoreIds || []}
              onIdsChange={onVectorStoreIdsChange}
              ownerType={vectorStoreOwnerType}
              ownerId={vectorStoreOwnerId}
              maxStores={5}
            />
            {/* Client-side RAG toggle - only show when vector stores are attached */}
            {vectorStoreIds && vectorStoreIds.length > 0 && onClientSideRAGChange && (
              <div className="mt-4 rounded-lg border p-4">
                <div className="flex items-center justify-between">
                  <div>
                    <span className="text-sm font-medium">Client-side search</span>
                    <p className="text-xs text-muted-foreground mt-0.5">
                      Execute searches in the browser instead of on the server. More transparent but
                      slower due to extra round-trips.
                    </p>
                  </div>
                  <Switch
                    checked={clientSideRAG ?? false}
                    onChange={(e) => onClientSideRAGChange(e.target.checked)}
                    aria-label="Client-side search"
                  />
                </div>
              </div>
            )}
          </div>
        )}

        {/* Response Actions Section */}
        {onActionConfigChange && (
          <div className="space-y-3 mb-6">
            <div className="flex items-center justify-between">
              <div>
                <h3 className="text-sm font-medium mb-1">Response Actions</h3>
                <p className="text-xs text-muted-foreground">
                  Configure which action buttons appear on response cards.
                </p>
              </div>
              <Button
                variant="ghost"
                size="sm"
                onClick={resetActionConfig}
                disabled={!hasActionConfigChanges}
                className="h-7 px-2 text-xs"
              >
                <RotateCcw className="mr-1 h-3 w-3" />
                Reset
              </Button>
            </div>
            <div className="rounded-lg border p-4 space-y-3">
              <div className="flex items-center justify-between">
                <span className="text-sm">Show select best button</span>
                <Switch
                  checked={actionConfig.showSelectBest ?? true}
                  onChange={(e) => updateActionConfig("showSelectBest", e.target.checked)}
                  aria-label="Show select best button"
                />
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm">Show regenerate button</span>
                <Switch
                  checked={actionConfig.showRegenerate ?? true}
                  onChange={(e) => updateActionConfig("showRegenerate", e.target.checked)}
                  aria-label="Show regenerate button"
                />
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm">Show copy button</span>
                <Switch
                  checked={actionConfig.showCopy ?? true}
                  onChange={(e) => updateActionConfig("showCopy", e.target.checked)}
                  aria-label="Show copy button"
                />
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm">Show expand button</span>
                <Switch
                  checked={actionConfig.showExpand ?? true}
                  onChange={(e) => updateActionConfig("showExpand", e.target.checked)}
                  aria-label="Show expand button"
                />
              </div>
            </div>
          </div>
        )}

        {/* Text-to-Speech Section */}
        {onTTSVoiceChange && onTTSSpeedChange && (
          <div className="space-y-3 mb-6">
            <div className="flex items-center justify-between">
              <div>
                <h3 className="text-sm font-medium mb-1 flex items-center gap-2">
                  <Volume2 className="h-4 w-4" />
                  Text-to-Speech
                </h3>
                <p className="text-xs text-muted-foreground">
                  Configure voice and playback speed for reading responses aloud.
                </p>
              </div>
              {(ttsVoice !== DEFAULT_TTS_VOICE || ttsSpeed !== DEFAULT_TTS_SPEED) && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => {
                    onTTSVoiceChange(DEFAULT_TTS_VOICE);
                    onTTSSpeedChange(DEFAULT_TTS_SPEED);
                  }}
                  className="h-7 px-2 text-xs"
                >
                  <RotateCcw className="mr-1 h-3 w-3" />
                  Reset
                </Button>
              )}
            </div>
            <div className="rounded-lg border p-4 space-y-4">
              <div className="space-y-2">
                <span className="text-sm">Voice</span>
                <Select
                  options={VOICE_OPTIONS}
                  value={ttsVoice}
                  onChange={(value) => value && onTTSVoiceChange(value)}
                  placeholder="Select voice..."
                  clearable={false}
                />
              </div>
              <Slider
                label="Playback Speed"
                value={ttsSpeed}
                onChange={onTTSSpeedChange}
                min={0.5}
                max={2}
                step={0.25}
                showValue
              />
            </div>
          </div>
        )}

        {/* Debug Section */}
        {onCaptureRawSSEEventsChange && (
          <div className="space-y-3 mb-6">
            <div>
              <h3 className="text-sm font-medium mb-1">Debug Options</h3>
              <p className="text-xs text-muted-foreground">
                Advanced options for debugging and development.
              </p>
            </div>
            <div className="rounded-lg border p-4 space-y-3">
              <div className="flex items-center justify-between">
                <div>
                  <span className="text-sm">Capture raw SSE events</span>
                  <p className="text-xs text-muted-foreground mt-0.5">
                    Record streaming events for inspection in the debug view. Can generate
                    significant data.
                  </p>
                </div>
                <Switch
                  checked={captureRawSSEEvents ?? false}
                  onChange={(e) => onCaptureRawSSEEventsChange(e.target.checked)}
                  aria-label="Capture raw SSE events"
                />
              </div>
            </div>
          </div>
        )}
      </ModalContent>

      <ModalFooter>
        <Button variant="outline" onClick={onClose}>
          Close
        </Button>
      </ModalFooter>

      {/* Prompt Form Modal */}
      <PromptFormModal
        open={promptFormOpen}
        onClose={() => setPromptFormOpen(false)}
        initialContent={systemPrompt}
        editingPrompt={editingPrompt}
        onSaved={handlePromptSaved}
      />
    </Modal>
  );
}
