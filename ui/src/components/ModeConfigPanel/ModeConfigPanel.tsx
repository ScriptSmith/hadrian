import { Settings2 } from "lucide-react";
import { useCallback } from "react";

import { Button } from "@/components/Button/Button";
import { Dropdown, DropdownContent, DropdownTrigger } from "@/components/Dropdown/Dropdown";
import { Label } from "@/components/Label/Label";
import { Textarea } from "@/components/Textarea/Textarea";
import type { ConversationMode, ModeConfig, ModelInstance } from "@/components/chat-types";
import { cn } from "@/utils/cn";

/**
 * Get display name for an instance.
 * Shows label if set, otherwise shows the model name (last part of modelId).
 * If there are multiple instances of the same model, appends "(instance N)" for unlabeled ones.
 */
function getInstanceDisplayName(instance: ModelInstance, allInstances: ModelInstance[]): string {
  if (instance.label) {
    return instance.label;
  }
  const modelName = instance.modelId.split("/").pop() || instance.modelId;
  // Check if there are other instances with the same modelId
  const sameModelInstances = allInstances.filter((i) => i.modelId === instance.modelId);
  if (sameModelInstances.length > 1) {
    const index = sameModelInstances.findIndex((i) => i.id === instance.id);
    return `${modelName} (${index + 1})`;
  }
  return modelName;
}

interface ModeConfigPanelProps {
  /** Current conversation mode */
  mode: ConversationMode;
  /** Current mode configuration */
  config: ModeConfig;
  /** Callback when configuration changes */
  onConfigChange: (config: ModeConfig) => void;
  /** Available instances for selection */
  availableInstances: ModelInstance[];
  /** Whether configuration changes are disabled (e.g., during streaming) */
  disabled?: boolean;
}

/**
 * ModeConfigPanel - Configure mode-specific settings
 *
 * Provides UI for configuring:
 * - Routed mode: router model, routing prompt
 * - Chained mode: chain order
 * - Synthesized mode: synthesizer model, synthesis prompt
 * - etc.
 */
export function ModeConfigPanel({
  mode,
  config,
  onConfigChange,
  availableInstances,
  disabled = false,
}: ModeConfigPanelProps) {
  // Only show for modes with configurable options
  const hasConfig = [
    "routed",
    "chained",
    "synthesized",
    "refined",
    "critiqued",
    "elected",
    "tournament",
    "consensus",
    "debated",
    "council",
    "hierarchical",
    "scattershot",
    "explainer",
    "confidence-weighted",
  ].includes(mode);

  if (!hasConfig) {
    return null;
  }

  return (
    <Dropdown>
      <DropdownTrigger
        disabled={disabled}
        showChevron={false}
        aria-label="Mode settings"
        className={cn(
          "h-8 w-8 rounded-lg border border-input bg-background p-0",
          "flex items-center justify-center",
          "transition-all duration-150",
          "hover:bg-accent hover:text-accent-foreground hover:border-accent",
          "focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
          "disabled:opacity-50 disabled:cursor-not-allowed"
        )}
      >
        <Settings2 className="h-4 w-4" />
      </DropdownTrigger>
      <DropdownContent align="end" className="w-80 p-3">
        <ConfigContent
          mode={mode}
          config={config}
          onConfigChange={onConfigChange}
          availableInstances={availableInstances}
          disabled={disabled}
        />
      </DropdownContent>
    </Dropdown>
  );
}

interface ConfigContentProps {
  mode: ConversationMode;
  config: ModeConfig;
  onConfigChange: (config: ModeConfig) => void;
  availableInstances: ModelInstance[];
  disabled: boolean;
}

function ConfigContent({
  mode,
  config,
  onConfigChange,
  availableInstances,
  disabled,
}: ConfigContentProps) {
  const updateConfig = useCallback(
    (updates: Partial<ModeConfig>) => {
      onConfigChange({ ...config, ...updates });
    },
    [config, onConfigChange]
  );

  switch (mode) {
    case "routed":
      return (
        <RoutedConfig
          config={config}
          onUpdate={updateConfig}
          availableInstances={availableInstances}
          disabled={disabled}
        />
      );
    case "chained":
      return (
        <ChainedConfig
          config={config}
          onUpdate={updateConfig}
          availableInstances={availableInstances}
          disabled={disabled}
        />
      );
    case "synthesized":
      return (
        <SynthesizedConfig
          config={config}
          onUpdate={updateConfig}
          availableInstances={availableInstances}
          disabled={disabled}
        />
      );
    case "refined":
      return (
        <RefinedConfig
          config={config}
          onUpdate={updateConfig}
          availableInstances={availableInstances}
          disabled={disabled}
        />
      );
    case "critiqued":
      return (
        <CritiquedConfig
          config={config}
          onUpdate={updateConfig}
          availableInstances={availableInstances}
          disabled={disabled}
        />
      );
    case "elected":
      return (
        <ElectedConfig
          config={config}
          onUpdate={updateConfig}
          availableInstances={availableInstances}
          disabled={disabled}
        />
      );
    case "tournament":
      return (
        <TournamentConfig
          config={config}
          onUpdate={updateConfig}
          availableInstances={availableInstances}
          disabled={disabled}
        />
      );
    case "consensus":
      return (
        <ConsensusConfig
          config={config}
          onUpdate={updateConfig}
          availableInstances={availableInstances}
          disabled={disabled}
        />
      );
    case "debated":
      return (
        <DebatedConfig
          config={config}
          onUpdate={updateConfig}
          availableInstances={availableInstances}
          disabled={disabled}
        />
      );
    case "council":
      return (
        <CouncilConfig
          config={config}
          onUpdate={updateConfig}
          availableInstances={availableInstances}
          disabled={disabled}
        />
      );
    case "hierarchical":
      return (
        <HierarchicalConfig
          config={config}
          onUpdate={updateConfig}
          availableInstances={availableInstances}
          disabled={disabled}
        />
      );
    case "scattershot":
      return (
        <ScattershotConfig
          config={config}
          onUpdate={updateConfig}
          availableInstances={availableInstances}
          disabled={disabled}
        />
      );
    case "explainer":
      return (
        <ExplainerConfig
          config={config}
          onUpdate={updateConfig}
          availableInstances={availableInstances}
          disabled={disabled}
        />
      );
    case "confidence-weighted":
      return (
        <ConfidenceWeightedConfig
          config={config}
          onUpdate={updateConfig}
          availableInstances={availableInstances}
          disabled={disabled}
        />
      );
    default:
      return (
        <p className="text-sm text-muted-foreground">Configuration for this mode is coming soon.</p>
      );
  }
}

interface ModeConfigProps {
  config: ModeConfig;
  onUpdate: (updates: Partial<ModeConfig>) => void;
  availableInstances: ModelInstance[];
  disabled: boolean;
}

/**
 * Get the currently selected instance ID from config.
 * Checks instanceId field first, then falls back to finding instance by modelId.
 */
function getSelectedInstanceId(
  instanceIdField: string | undefined,
  modelIdField: string | undefined,
  instances: ModelInstance[]
): string {
  // If instanceId is set, use it directly
  if (instanceIdField) {
    return instanceIdField;
  }
  // Fall back to finding instance by modelId (backwards compat)
  if (modelIdField) {
    const instance = instances.find((i) => i.modelId === modelIdField);
    if (instance) {
      return instance.id;
    }
  }
  return "";
}

function RoutedConfig({ config, onUpdate, availableInstances, disabled }: ModeConfigProps) {
  const selectedId = getSelectedInstanceId(
    config.routerInstanceId,
    config.routerModel,
    availableInstances
  );

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="routerModel" className="text-xs">
          Router Model
        </Label>
        <select
          id="routerModel"
          value={selectedId}
          onChange={(e) => {
            const instanceId = e.target.value || undefined;
            const instance = availableInstances.find((i) => i.id === instanceId);
            onUpdate({
              routerInstanceId: instanceId,
              routerModel: instance?.modelId,
            });
          }}
          disabled={disabled}
          className={cn(
            "flex h-9 w-full rounded-md border border-input bg-background px-3 py-1",
            "text-sm ring-offset-background",
            "focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
            "disabled:cursor-not-allowed disabled:opacity-50"
          )}
        >
          <option value="">First selected model (default)</option>
          {availableInstances.map((instance) => (
            <option key={instance.id} value={instance.id}>
              {getInstanceDisplayName(instance, availableInstances)}
            </option>
          ))}
        </select>
        <p className="text-[10px] text-muted-foreground">
          The model that analyzes prompts and selects the best responder.
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="routingPrompt" className="text-xs">
          Routing Prompt
        </Label>
        <Textarea
          id="routingPrompt"
          value={config.routingPrompt || ""}
          onChange={(e) => onUpdate({ routingPrompt: e.target.value || undefined })}
          disabled={disabled}
          placeholder="Leave empty to use default prompt. Use {models} to insert the list of available models."
          className="min-h-[100px] text-xs"
        />
        <p className="text-[10px] text-muted-foreground">
          Custom instructions for how the router should select a model. Use {"{{models}}"} as a
          placeholder.
        </p>
      </div>

      <Button
        variant="ghost"
        size="sm"
        onClick={() =>
          onUpdate({
            routerInstanceId: undefined,
            routerModel: undefined,
            routingPrompt: undefined,
          })
        }
        disabled={disabled}
        className="text-xs"
      >
        Reset to defaults
      </Button>
    </div>
  );
}

function ChainedConfig({ config, availableInstances }: ModeConfigProps) {
  // For chain order display, use instance labels or model names
  const displayOrder =
    config.chainOrder ||
    availableInstances.map((i) => getInstanceDisplayName(i, availableInstances));

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label className="text-xs">Chain Order</Label>
        <p className="text-[10px] text-muted-foreground">
          Models respond in the order they are selected. Drag models in the model selector to
          reorder them.
        </p>
      </div>

      <div className="space-y-2">
        <Label className="text-xs">Current Order</Label>
        <div className="flex flex-wrap gap-1">
          {displayOrder.map((name, index) => (
            <span
              key={`${name}-${index}`}
              className="text-[10px] px-1.5 py-0.5 rounded bg-muted text-muted-foreground"
            >
              {index + 1}. {name.split("/").pop()}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}

function SynthesizedConfig({ config, onUpdate, availableInstances, disabled }: ModeConfigProps) {
  const selectedId = getSelectedInstanceId(
    config.synthesizerInstanceId,
    config.synthesizerModel,
    availableInstances
  );

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="synthesizerModel" className="text-xs">
          Synthesizer Model
        </Label>
        <select
          id="synthesizerModel"
          value={selectedId}
          onChange={(e) => {
            const instanceId = e.target.value || undefined;
            const instance = availableInstances.find((i) => i.id === instanceId);
            onUpdate({
              synthesizerInstanceId: instanceId,
              synthesizerModel: instance?.modelId,
            });
          }}
          disabled={disabled}
          className={cn(
            "flex h-9 w-full rounded-md border border-input bg-background px-3 py-1",
            "text-sm ring-offset-background",
            "focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
            "disabled:cursor-not-allowed disabled:opacity-50"
          )}
        >
          <option value="">First selected model (default)</option>
          {availableInstances.map((instance) => (
            <option key={instance.id} value={instance.id}>
              {getInstanceDisplayName(instance, availableInstances)}
            </option>
          ))}
        </select>
        <p className="text-[10px] text-muted-foreground">
          The model that combines all responses into a unified answer.
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="synthesisPrompt" className="text-xs">
          Synthesis Prompt
        </Label>
        <Textarea
          id="synthesisPrompt"
          value={config.synthesisPrompt || ""}
          onChange={(e) => onUpdate({ synthesisPrompt: e.target.value || undefined })}
          disabled={disabled}
          placeholder="Leave empty to use default synthesis prompt."
          className="min-h-[100px] text-xs"
        />
      </div>

      <Button
        variant="ghost"
        size="sm"
        onClick={() =>
          onUpdate({
            synthesizerInstanceId: undefined,
            synthesizerModel: undefined,
            synthesisPrompt: undefined,
          })
        }
        disabled={disabled}
        className="text-xs"
      >
        Reset to defaults
      </Button>
    </div>
  );
}

function RefinedConfig({ config, onUpdate, availableInstances, disabled }: ModeConfigProps) {
  const maxRounds = availableInstances.length;

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="refinementRounds" className="text-xs">
          Refinement Rounds
        </Label>
        <select
          id="refinementRounds"
          value={config.refinementRounds || 2}
          onChange={(e) => onUpdate({ refinementRounds: parseInt(e.target.value) })}
          disabled={disabled}
          className={cn(
            "flex h-9 w-full rounded-md border border-input bg-background px-3 py-1",
            "text-sm ring-offset-background",
            "focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
            "disabled:cursor-not-allowed disabled:opacity-50"
          )}
        >
          {Array.from({ length: maxRounds }, (_, i) => i + 1).map((num) => (
            <option key={num} value={num}>
              {num} round{num > 1 ? "s" : ""}
              {num === 2 && " (default)"}
            </option>
          ))}
        </select>
        <p className="text-[10px] text-muted-foreground">
          How many times to refine the response. Each round uses the next model in order.
        </p>
      </div>

      <div className="space-y-2">
        <Label className="text-xs">Model Order</Label>
        <div className="flex flex-wrap gap-1">
          {availableInstances.slice(0, config.refinementRounds || 2).map((instance, index) => (
            <span
              key={instance.id}
              className="text-[10px] px-1.5 py-0.5 rounded bg-muted text-muted-foreground"
            >
              {index === 0 ? "Initial" : `Refine ${index}`}:{" "}
              {getInstanceDisplayName(instance, availableInstances)}
            </span>
          ))}
        </div>
        <p className="text-[10px] text-muted-foreground">
          First model generates initial response, subsequent models refine it.
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="refinementPrompt" className="text-xs">
          Refinement Prompt
        </Label>
        <Textarea
          id="refinementPrompt"
          value={config.refinementPrompt || ""}
          onChange={(e) => onUpdate({ refinementPrompt: e.target.value || undefined })}
          disabled={disabled}
          placeholder="Leave empty to use default refinement prompt. Use {previous_response} to insert the response to refine."
          className="min-h-[100px] text-xs"
        />
        <p className="text-[10px] text-muted-foreground">
          Custom instructions for how models should refine responses. Use {"{{previous_response}}"}{" "}
          as a placeholder.
        </p>
      </div>

      <Button
        variant="ghost"
        size="sm"
        onClick={() => onUpdate({ refinementRounds: 2, refinementPrompt: undefined })}
        disabled={disabled}
        className="text-xs"
      >
        Reset to defaults
      </Button>
    </div>
  );
}

function CritiquedConfig({ config, onUpdate, availableInstances, disabled }: ModeConfigProps) {
  const selectedId = getSelectedInstanceId(
    config.primaryInstanceId,
    config.primaryModel,
    availableInstances
  );
  // Find the primary instance - use selected or default to first
  const primaryInstance = selectedId
    ? availableInstances.find((i) => i.id === selectedId)
    : availableInstances[0];
  const critiqueInstances = availableInstances.filter((i) => i.id !== primaryInstance?.id);

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="primaryModel" className="text-xs">
          Primary Model
        </Label>
        <select
          id="primaryModel"
          value={selectedId}
          onChange={(e) => {
            const instanceId = e.target.value || undefined;
            const instance = availableInstances.find((i) => i.id === instanceId);
            onUpdate({
              primaryInstanceId: instanceId,
              primaryModel: instance?.modelId,
            });
          }}
          disabled={disabled}
          className={cn(
            "flex h-9 w-full rounded-md border border-input bg-background px-3 py-1",
            "text-sm ring-offset-background",
            "focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
            "disabled:cursor-not-allowed disabled:opacity-50"
          )}
        >
          <option value="">First selected model (default)</option>
          {availableInstances.map((instance) => (
            <option key={instance.id} value={instance.id}>
              {getInstanceDisplayName(instance, availableInstances)}
            </option>
          ))}
        </select>
        <p className="text-[10px] text-muted-foreground">
          The model that provides the initial response and revision.
        </p>
      </div>

      <div className="space-y-2">
        <Label className="text-xs">Critic Models</Label>
        <div className="flex flex-wrap gap-1">
          {critiqueInstances.map((instance) => (
            <span
              key={instance.id}
              className="text-[10px] px-1.5 py-0.5 rounded bg-orange-500/20 text-orange-800 dark:text-orange-400"
            >
              {getInstanceDisplayName(instance, availableInstances)}
            </span>
          ))}
        </div>
        <p className="text-[10px] text-muted-foreground">
          All other selected models will provide critiques.
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="critiquePrompt" className="text-xs">
          Critique Prompt
        </Label>
        <Textarea
          id="critiquePrompt"
          value={config.critiquePrompt || ""}
          onChange={(e) => onUpdate({ critiquePrompt: e.target.value || undefined })}
          disabled={disabled}
          placeholder="Leave empty to use default critique prompt. Use {response} to insert the response to critique."
          className="min-h-[100px] text-xs"
        />
        <p className="text-[10px] text-muted-foreground">
          Custom instructions for how critics should provide feedback. Use {"{{response}}"} as a
          placeholder.
        </p>
      </div>

      <Button
        variant="ghost"
        size="sm"
        onClick={() =>
          onUpdate({
            primaryInstanceId: undefined,
            primaryModel: undefined,
            critiquePrompt: undefined,
          })
        }
        disabled={disabled}
        className="text-xs"
      >
        Reset to defaults
      </Button>
    </div>
  );
}

function ElectedConfig({ config, onUpdate, availableInstances, disabled }: ModeConfigProps) {
  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label className="text-xs">Candidates</Label>
        <div className="flex flex-wrap gap-1">
          {availableInstances.map((instance) => (
            <span
              key={instance.id}
              className="text-[10px] px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-700 dark:text-blue-400"
            >
              {getInstanceDisplayName(instance, availableInstances)}
            </span>
          ))}
        </div>
        <p className="text-[10px] text-muted-foreground">
          All selected models will respond and then vote on the best response.
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="votingPrompt" className="text-xs">
          Voting Prompt
        </Label>
        <Textarea
          id="votingPrompt"
          value={config.votingPrompt || ""}
          onChange={(e) => onUpdate({ votingPrompt: e.target.value || undefined })}
          disabled={disabled}
          placeholder="Leave empty to use default voting prompt. Use {question} for the user's question and {candidates} for the list of responses."
          className="min-h-[100px] text-xs"
        />
        <p className="text-[10px] text-muted-foreground">
          Custom instructions for how models should evaluate and vote. Use {"{{question}}"} and{" "}
          {"{{candidates}}"} as placeholders.
        </p>
      </div>

      <Button
        variant="ghost"
        size="sm"
        onClick={() => onUpdate({ votingPrompt: undefined })}
        disabled={disabled}
        className="text-xs"
      >
        Reset to defaults
      </Button>
    </div>
  );
}

function TournamentConfig({ config, onUpdate, availableInstances, disabled }: ModeConfigProps) {
  const numRounds = Math.ceil(Math.log2(availableInstances.length));
  const selectedId = getSelectedInstanceId(
    config.primaryInstanceId,
    config.primaryModel,
    availableInstances
  );

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label className="text-xs">Competitors</Label>
        <div className="flex flex-wrap gap-1">
          {availableInstances.map((instance) => (
            <span
              key={instance.id}
              className="text-[10px] px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-700 dark:text-blue-400"
            >
              {getInstanceDisplayName(instance, availableInstances)}
            </span>
          ))}
        </div>
        <p className="text-[10px] text-muted-foreground">
          {availableInstances.length} models will compete in {numRounds} round
          {numRounds > 1 ? "s" : ""}.
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="judgeModel" className="text-xs">
          Judge Model
        </Label>
        <select
          id="judgeModel"
          value={selectedId}
          onChange={(e) => {
            const instanceId = e.target.value || undefined;
            const instance = availableInstances.find((i) => i.id === instanceId);
            onUpdate({
              primaryInstanceId: instanceId,
              primaryModel: instance?.modelId,
            });
          }}
          disabled={disabled}
          className={cn(
            "flex h-9 w-full rounded-md border border-input bg-background px-3 py-1",
            "text-sm ring-offset-background",
            "focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
            "disabled:cursor-not-allowed disabled:opacity-50"
          )}
        >
          <option value="">Auto-select (not in match)</option>
          {availableInstances.map((instance) => (
            <option key={instance.id} value={instance.id}>
              {getInstanceDisplayName(instance, availableInstances)}
            </option>
          ))}
        </select>
        <p className="text-[10px] text-muted-foreground">
          The model that judges each match. By default, a model not in the current match is
          selected.
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="judgingPrompt" className="text-xs">
          Judging Prompt
        </Label>
        <Textarea
          id="judgingPrompt"
          value={config.votingPrompt || ""}
          onChange={(e) => onUpdate({ votingPrompt: e.target.value || undefined })}
          disabled={disabled}
          placeholder="Leave empty to use default judging prompt. Use {question}, {response_a}, and {response_b} as placeholders."
          className="min-h-[100px] text-xs"
        />
        <p className="text-[10px] text-muted-foreground">
          Custom instructions for how the judge compares responses. Use {"{{question}}"},{" "}
          {"{{response_a}}"}, and {"{{response_b}}"} as placeholders.
        </p>
      </div>

      <Button
        variant="ghost"
        size="sm"
        onClick={() =>
          onUpdate({
            primaryInstanceId: undefined,
            primaryModel: undefined,
            votingPrompt: undefined,
          })
        }
        disabled={disabled}
        className="text-xs"
      >
        Reset to defaults
      </Button>
    </div>
  );
}

function ConsensusConfig({ config, onUpdate, availableInstances, disabled }: ModeConfigProps) {
  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label className="text-xs">Participants</Label>
        <div className="flex flex-wrap gap-1">
          {availableInstances.map((instance) => (
            <span
              key={instance.id}
              className="text-[10px] px-1.5 py-0.5 rounded bg-primary/10 text-primary"
            >
              {getInstanceDisplayName(instance, availableInstances)}
            </span>
          ))}
        </div>
        <p className="text-[10px] text-muted-foreground">
          All selected models will participate in building consensus.
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="maxConsensusRounds" className="text-xs">
          Maximum Rounds
        </Label>
        <select
          id="maxConsensusRounds"
          value={config.maxConsensusRounds || 5}
          onChange={(e) => onUpdate({ maxConsensusRounds: parseInt(e.target.value) })}
          disabled={disabled}
          className={cn(
            "flex h-9 w-full rounded-md border border-input bg-background px-3 py-1",
            "text-sm ring-offset-background",
            "focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
            "disabled:cursor-not-allowed disabled:opacity-50"
          )}
        >
          {[2, 3, 4, 5, 6, 7, 8, 10].map((num) => (
            <option key={num} value={num}>
              {num} rounds{num === 5 && " (default)"}
            </option>
          ))}
        </select>
        <p className="text-[10px] text-muted-foreground">
          Maximum revision rounds before stopping. More rounds may yield better consensus.
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="consensusThreshold" className="text-xs">
          Consensus Threshold
        </Label>
        <select
          id="consensusThreshold"
          value={config.consensusThreshold || 0.8}
          onChange={(e) => onUpdate({ consensusThreshold: parseFloat(e.target.value) })}
          disabled={disabled}
          className={cn(
            "flex h-9 w-full rounded-md border border-input bg-background px-3 py-1",
            "text-sm ring-offset-background",
            "focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
            "disabled:cursor-not-allowed disabled:opacity-50"
          )}
        >
          <option value={0.6}>60% (loose)</option>
          <option value={0.7}>70%</option>
          <option value={0.8}>80% (default)</option>
          <option value={0.9}>90% (strict)</option>
          <option value={0.95}>95% (very strict)</option>
        </select>
        <p className="text-[10px] text-muted-foreground">
          Agreement level required to stop early. Higher values require more similar responses.
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="consensusPrompt" className="text-xs">
          Consensus Prompt
        </Label>
        <Textarea
          id="consensusPrompt"
          value={config.consensusPrompt || ""}
          onChange={(e) => onUpdate({ consensusPrompt: e.target.value || undefined })}
          disabled={disabled}
          placeholder="Leave empty to use default consensus prompt. Use {question} for the original question and {responses} for all current responses."
          className="min-h-[100px] text-xs"
        />
        <p className="text-[10px] text-muted-foreground">
          Custom instructions for how models should revise toward consensus. Use {"{{question}}"}{" "}
          and {"{{responses}}"} as placeholders.
        </p>
      </div>

      <Button
        variant="ghost"
        size="sm"
        onClick={() =>
          onUpdate({
            maxConsensusRounds: 5,
            consensusThreshold: 0.8,
            consensusPrompt: undefined,
          })
        }
        disabled={disabled}
        className="text-xs"
      >
        Reset to defaults
      </Button>
    </div>
  );
}

function DebatedConfig({ config, onUpdate, availableInstances, disabled }: ModeConfigProps) {
  const selectedId = getSelectedInstanceId(
    config.synthesizerInstanceId,
    config.synthesizerModel,
    availableInstances
  );

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label className="text-xs">Debaters & Positions</Label>
        <div className="flex flex-wrap gap-1">
          {availableInstances.map((instance, index) => {
            const position = index % 2 === 0 ? "pro" : "con";
            return (
              <span
                key={instance.id}
                className={cn(
                  "text-[10px] px-1.5 py-0.5 rounded font-medium",
                  position === "pro"
                    ? "bg-emerald-500/20 text-emerald-800 dark:text-emerald-400"
                    : "bg-rose-500/20 text-rose-800 dark:text-rose-400"
                )}
              >
                {getInstanceDisplayName(instance, availableInstances)} ({position})
              </span>
            );
          })}
        </div>
        <p className="text-[10px] text-muted-foreground">
          Models are automatically assigned alternating pro/con positions.
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="debateRounds" className="text-xs">
          Debate Rounds
        </Label>
        <select
          id="debateRounds"
          value={config.debateRounds || 3}
          onChange={(e) => onUpdate({ debateRounds: parseInt(e.target.value) })}
          disabled={disabled}
          className={cn(
            "flex h-9 w-full rounded-md border border-input bg-background px-3 py-1",
            "text-sm ring-offset-background",
            "focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
            "disabled:cursor-not-allowed disabled:opacity-50"
          )}
        >
          {[1, 2, 3, 4, 5].map((num) => (
            <option key={num} value={num}>
              {num} round{num > 1 ? "s" : ""}
              {num === 3 && " (default)"}
            </option>
          ))}
        </select>
        <p className="text-[10px] text-muted-foreground">
          Number of rebuttal rounds after opening statements. More rounds allow deeper discussion.
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="summarizerModel" className="text-xs">
          Summarizer Model
        </Label>
        <select
          id="summarizerModel"
          value={selectedId}
          onChange={(e) => {
            const instanceId = e.target.value || undefined;
            const instance = availableInstances.find((i) => i.id === instanceId);
            onUpdate({
              synthesizerInstanceId: instanceId,
              synthesizerModel: instance?.modelId,
            });
          }}
          disabled={disabled}
          className={cn(
            "flex h-9 w-full rounded-md border border-input bg-background px-3 py-1",
            "text-sm ring-offset-background",
            "focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
            "disabled:cursor-not-allowed disabled:opacity-50"
          )}
        >
          <option value="">First selected model (default)</option>
          {availableInstances.map((instance) => (
            <option key={instance.id} value={instance.id}>
              {getInstanceDisplayName(instance, availableInstances)}
            </option>
          ))}
        </select>
        <p className="text-[10px] text-muted-foreground">
          The model that synthesizes the debate into a balanced summary.
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="debatePrompt" className="text-xs">
          Debate Prompt
        </Label>
        <Textarea
          id="debatePrompt"
          value={config.debatePrompt || ""}
          onChange={(e) => onUpdate({ debatePrompt: e.target.value || undefined })}
          disabled={disabled}
          placeholder="Leave empty to use default debate prompts. Use {position} for the model's position and {question} for the user's question."
          className="min-h-[100px] text-xs"
        />
        <p className="text-[10px] text-muted-foreground">
          Custom instructions for debate arguments. Use {"{{position}}"} and {"{{question}}"} as
          placeholders.
        </p>
      </div>

      <Button
        variant="ghost"
        size="sm"
        onClick={() =>
          onUpdate({
            debateRounds: 3,
            synthesizerInstanceId: undefined,
            synthesizerModel: undefined,
            debatePrompt: undefined,
          })
        }
        disabled={disabled}
        className="text-xs"
      >
        Reset to defaults
      </Button>
    </div>
  );
}

const DEFAULT_COUNCIL_ROLES = [
  "Technical Expert",
  "Business Analyst",
  "User Advocate",
  "Risk Assessor",
  "Innovation Specialist",
  "Quality Assurance",
  "Operations Lead",
  "Strategy Advisor",
];

function CouncilConfig({ config, onUpdate, availableInstances, disabled }: ModeConfigProps) {
  const selectedId = getSelectedInstanceId(
    config.synthesizerInstanceId,
    config.synthesizerModel,
    availableInstances
  );
  // Find the synthesizer instance - use selected or default to first
  const synthesizerInstance = selectedId
    ? availableInstances.find((i) => i.id === selectedId)
    : availableInstances[0];
  // Council members are all instances except the synthesizer
  const councilMembers = availableInstances.filter((i) => i.id !== synthesizerInstance?.id);

  // Get current role assignments (now keyed by instance ID)
  const getRoleForInstance = (instance: ModelInstance, index: number): string => {
    return (
      config.councilRoles?.[instance.id] ||
      DEFAULT_COUNCIL_ROLES[index % DEFAULT_COUNCIL_ROLES.length]
    );
  };

  const updateRole = (instanceId: string, role: string) => {
    const newRoles = { ...(config.councilRoles || {}) };
    if (role) {
      newRoles[instanceId] = role;
    } else {
      delete newRoles[instanceId];
    }
    onUpdate({ councilRoles: Object.keys(newRoles).length > 0 ? newRoles : undefined });
  };

  const autoAssignRoles = config.councilAutoAssignRoles ?? false;

  return (
    <div className="space-y-4">
      {/* Synthesizer Model - First, since it determines council members */}
      <div className="space-y-2">
        <Label htmlFor="synthesizerModel" className="text-xs">
          Synthesizer Model
        </Label>
        <select
          id="synthesizerModel"
          value={selectedId}
          onChange={(e) => {
            const instanceId = e.target.value || undefined;
            const instance = availableInstances.find((i) => i.id === instanceId);
            onUpdate({
              synthesizerInstanceId: instanceId,
              synthesizerModel: instance?.modelId,
            });
          }}
          disabled={disabled}
          className={cn(
            "flex h-9 w-full rounded-md border border-input bg-background px-3 py-1",
            "text-sm ring-offset-background",
            "focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
            "disabled:cursor-not-allowed disabled:opacity-50"
          )}
        >
          <option value="">First selected model (default)</option>
          {availableInstances.map((instance) => (
            <option key={instance.id} value={instance.id}>
              {getInstanceDisplayName(instance, availableInstances)}
            </option>
          ))}
        </select>
        <p className="text-[10px] text-muted-foreground">
          Observes the discussion and synthesizes all perspectives. Does not participate in the
          council.
        </p>
      </div>

      {/* Auto-assign roles toggle */}
      <div className="flex items-center justify-between">
        <div className="space-y-0.5">
          <Label className="text-xs">Auto-assign Roles</Label>
          <p className="text-[10px] text-muted-foreground">
            Let the synthesizer assign roles based on the question
          </p>
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={autoAssignRoles}
          onClick={() => onUpdate({ councilAutoAssignRoles: !autoAssignRoles })}
          disabled={disabled}
          className={cn(
            "relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent transition-colors",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
            "disabled:cursor-not-allowed disabled:opacity-50",
            autoAssignRoles ? "bg-primary" : "bg-input"
          )}
        >
          <span
            className={cn(
              "pointer-events-none block h-4 w-4 rounded-full bg-background shadow-lg ring-0 transition-transform",
              autoAssignRoles ? "translate-x-4" : "translate-x-0"
            )}
          />
        </button>
      </div>

      {/* Council Members & Roles - Only show if not auto-assigning */}
      {!autoAssignRoles && (
        <div className="space-y-2">
          <Label className="text-xs">Council Members & Roles</Label>
          {councilMembers.length === 0 ? (
            <p className="text-[10px] text-muted-foreground italic">
              Select at least 2 models to have council members besides the synthesizer.
            </p>
          ) : (
            <div className="space-y-2">
              {councilMembers.map((instance, index) => (
                <div key={instance.id} className="flex items-center gap-2">
                  <span className="text-[10px] px-1.5 py-0.5 rounded bg-muted text-muted-foreground min-w-[70px] max-w-[70px] truncate">
                    {getInstanceDisplayName(instance, availableInstances)}
                  </span>
                  <input
                    type="text"
                    value={getRoleForInstance(instance, index)}
                    onChange={(e) => updateRole(instance.id, e.target.value)}
                    disabled={disabled}
                    placeholder="Enter role..."
                    aria-label={`Role for ${getInstanceDisplayName(instance, availableInstances)}`}
                    list={`role-suggestions-${index}`}
                    className={cn(
                      "flex-1 h-7 rounded-md border border-input bg-background px-2",
                      "text-xs ring-offset-background",
                      "focus:outline-none focus:ring-1 focus:ring-ring",
                      "disabled:cursor-not-allowed disabled:opacity-50"
                    )}
                  />
                  <datalist id={`role-suggestions-${index}`}>
                    {DEFAULT_COUNCIL_ROLES.map((role) => (
                      <option key={role} value={role} />
                    ))}
                  </datalist>
                </div>
              ))}
            </div>
          )}
          <p className="text-[10px] text-muted-foreground">
            Each council member will discuss from their assigned perspective. Type any role or
            select from suggestions.
          </p>
        </div>
      )}

      {/* Discussion Rounds */}
      <div className="space-y-2">
        <Label htmlFor="councilRounds" className="text-xs">
          Discussion Rounds
        </Label>
        <select
          id="councilRounds"
          value={config.debateRounds || 2}
          onChange={(e) => onUpdate({ debateRounds: parseInt(e.target.value) })}
          disabled={disabled}
          className={cn(
            "flex h-9 w-full rounded-md border border-input bg-background px-3 py-1",
            "text-sm ring-offset-background",
            "focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
            "disabled:cursor-not-allowed disabled:opacity-50"
          )}
        >
          {[1, 2, 3, 4, 5].map((num) => (
            <option key={num} value={num}>
              {num} round{num > 1 ? "s" : ""}
              {num === 2 && " (default)"}
            </option>
          ))}
        </select>
        <p className="text-[10px] text-muted-foreground">
          Number of discussion rounds after opening perspectives.
        </p>
      </div>

      {/* Custom Council Prompt */}
      <div className="space-y-2">
        <Label htmlFor="councilPrompt" className="text-xs">
          Council Prompt
        </Label>
        <Textarea
          id="councilPrompt"
          value={config.councilPrompt || ""}
          onChange={(e) => onUpdate({ councilPrompt: e.target.value || undefined })}
          disabled={disabled}
          placeholder="Leave empty to use default council prompts. Use {role} for the model's role and {question} for the user's question."
          className="min-h-[80px] text-xs"
        />
        <p className="text-[10px] text-muted-foreground">
          Custom instructions for council discussions. Use {"{{role}}"} and {"{{question}}"} as
          placeholders.
        </p>
      </div>

      <Button
        variant="ghost"
        size="sm"
        onClick={() =>
          onUpdate({
            debateRounds: 2,
            synthesizerInstanceId: undefined,
            synthesizerModel: undefined,
            councilPrompt: undefined,
            councilRoles: undefined,
            councilAutoAssignRoles: false,
          })
        }
        disabled={disabled}
        className="text-xs"
      >
        Reset to defaults
      </Button>
    </div>
  );
}

function HierarchicalConfig({ config, onUpdate, availableInstances, disabled }: ModeConfigProps) {
  const selectedId = getSelectedInstanceId(
    config.coordinatorInstanceId,
    config.coordinatorModel,
    availableInstances
  );
  // Find the coordinator instance - use selected or default to first
  const coordinatorInstance = selectedId
    ? availableInstances.find((i) => i.id === selectedId)
    : availableInstances[0];
  const workerInstances = availableInstances.filter((i) => i.id !== coordinatorInstance?.id);

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="coordinatorModel" className="text-xs">
          Coordinator Model
        </Label>
        <select
          id="coordinatorModel"
          value={selectedId}
          onChange={(e) => {
            const instanceId = e.target.value || undefined;
            const instance = availableInstances.find((i) => i.id === instanceId);
            onUpdate({
              coordinatorInstanceId: instanceId,
              coordinatorModel: instance?.modelId,
            });
          }}
          disabled={disabled}
          className={cn(
            "flex h-9 w-full rounded-md border border-input bg-background px-3 py-1",
            "text-sm ring-offset-background",
            "focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
            "disabled:cursor-not-allowed disabled:opacity-50"
          )}
        >
          <option value="">First selected model (default)</option>
          {availableInstances.map((instance) => (
            <option key={instance.id} value={instance.id}>
              {getInstanceDisplayName(instance, availableInstances)}
            </option>
          ))}
        </select>
        <p className="text-[10px] text-muted-foreground">
          Breaks down tasks and synthesizes results. Does not perform worker tasks.
        </p>
      </div>

      <div className="space-y-2">
        <Label className="text-xs">Worker Models</Label>
        {workerInstances.length === 0 ? (
          <p className="text-[10px] text-muted-foreground italic">
            Select at least 2 models to have workers besides the coordinator.
          </p>
        ) : (
          <div className="flex flex-wrap gap-1">
            {workerInstances.map((instance) => (
              <span
                key={instance.id}
                className="text-[10px] px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-700 dark:text-blue-400"
              >
                {getInstanceDisplayName(instance, availableInstances)}
              </span>
            ))}
          </div>
        )}
        <p className="text-[10px] text-muted-foreground">
          Workers are assigned subtasks by the coordinator and execute them in parallel.
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="decompositionPrompt" className="text-xs">
          Decomposition Prompt
        </Label>
        <Textarea
          id="decompositionPrompt"
          value={config.routingPrompt || ""}
          onChange={(e) => onUpdate({ routingPrompt: e.target.value || undefined })}
          disabled={disabled}
          placeholder="Leave empty to use default. Use {question} for the user's question, {workers} for available workers, and {count} for worker count."
          className="min-h-[80px] text-xs"
        />
        <p className="text-[10px] text-muted-foreground">
          Instructions for how the coordinator breaks down tasks. Use {"{{question}}"},{" "}
          {"{{workers}}"}, {"{{count}}"} as placeholders.
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="workerPrompt" className="text-xs">
          Worker Prompt
        </Label>
        <Textarea
          id="workerPrompt"
          value={config.hierarchicalWorkerPrompt || ""}
          onChange={(e) => onUpdate({ hierarchicalWorkerPrompt: e.target.value || undefined })}
          disabled={disabled}
          placeholder="Leave empty to use default. Use {task} for the subtask description and {context} for the original question."
          className="min-h-[80px] text-xs"
        />
        <p className="text-[10px] text-muted-foreground">
          Instructions for workers completing subtasks. Use {"{{task}}"} and {"{{context}}"} as
          placeholders.
        </p>
      </div>

      <Button
        variant="ghost"
        size="sm"
        onClick={() =>
          onUpdate({
            coordinatorInstanceId: undefined,
            coordinatorModel: undefined,
            routingPrompt: undefined,
            hierarchicalWorkerPrompt: undefined,
          })
        }
        disabled={disabled}
        className="text-xs"
      >
        Reset to defaults
      </Button>
    </div>
  );
}

const DEFAULT_TEMPERATURE_PRESETS = [
  { label: "Deterministic (0.0)", temp: 0.0, topP: undefined },
  { label: "Conservative (0.3)", temp: 0.3, topP: undefined },
  { label: "Balanced (0.5)", temp: 0.5, topP: undefined },
  { label: "Creative (0.7)", temp: 0.7, topP: undefined },
  { label: "Very Creative (1.0)", temp: 1.0, topP: undefined },
  { label: "Experimental (1.5)", temp: 1.5, topP: 0.9 },
];

function ScattershotConfig({ config, onUpdate, availableInstances, disabled }: ModeConfigProps) {
  const targetInstance = availableInstances[0];

  // Get current variations or build from presets
  const currentVariations = config.parameterVariations || [
    { temperature: 0.0 },
    { temperature: 0.5 },
    { temperature: 1.0 },
    { temperature: 1.5, topP: 0.9 },
  ];

  // Toggle a preset variation
  const togglePreset = (temp: number, topP: number | undefined) => {
    const current = config.parameterVariations || [];
    const existing = current.findIndex((v) => v.temperature === temp && v.topP === topP);

    if (existing !== -1) {
      // Remove if already selected
      const updated = [...current];
      updated.splice(existing, 1);
      onUpdate({ parameterVariations: updated.length > 0 ? updated : undefined });
    } else {
      // Add new variation
      const updated = [...current, { temperature: temp, topP }];
      onUpdate({ parameterVariations: updated });
    }
  };

  const isPresetSelected = (temp: number, topP: number | undefined) => {
    return currentVariations.some((v) => v.temperature === temp && v.topP === topP);
  };

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label className="text-xs">Target Model</Label>
        <div className="px-1.5 py-1 rounded bg-primary/10 text-primary font-medium text-[11px] inline-block">
          {targetInstance
            ? getInstanceDisplayName(targetInstance, availableInstances)
            : "Select a model"}
        </div>
        <p className="text-[10px] text-muted-foreground">
          Scattershot uses only the first selected model with varied parameters.
        </p>
      </div>

      <div className="space-y-2">
        <Label className="text-xs">
          Parameter Variations ({currentVariations.length} selected)
        </Label>
        <div className="space-y-1">
          {DEFAULT_TEMPERATURE_PRESETS.map((preset) => (
            <button
              key={preset.label}
              type="button"
              onClick={() => togglePreset(preset.temp, preset.topP)}
              disabled={disabled}
              className={cn(
                "w-full text-left px-2 py-1.5 rounded text-[11px] transition-colors",
                "border border-transparent",
                isPresetSelected(preset.temp, preset.topP)
                  ? "bg-primary/10 text-primary border-primary/30"
                  : "bg-muted/50 text-muted-foreground hover:bg-muted hover:text-foreground",
                disabled && "opacity-50 cursor-not-allowed"
              )}
            >
              <div className="flex items-center justify-between">
                <span>{preset.label}</span>
                {preset.topP && (
                  <span className="text-[9px] text-muted-foreground">top_p={preset.topP}</span>
                )}
              </div>
            </button>
          ))}
        </div>
        <p className="text-[10px] text-muted-foreground">
          Select 2+ variations to compare responses with different temperature settings.
        </p>
      </div>

      {currentVariations.length < 2 && (
        <div className="text-[10px] text-orange-500 bg-orange-500/10 px-2 py-1.5 rounded">
          Select at least 2 variations to enable scattershot mode.
        </div>
      )}

      <Button
        variant="ghost"
        size="sm"
        onClick={() => onUpdate({ parameterVariations: undefined })}
        disabled={disabled}
        className="text-xs"
      >
        Reset to defaults
      </Button>
    </div>
  );
}

const DEFAULT_AUDIENCE_LEVEL_OPTIONS = [
  { id: "expert", label: "Expert", description: "Technical, precise, assumes deep knowledge" },
  { id: "intermediate", label: "Intermediate", description: "Working knowledge, some jargon" },
  { id: "beginner", label: "Beginner", description: "Simple language, step by step" },
  { id: "non-technical", label: "Non-Technical", description: "No jargon, everyday language" },
  { id: "child", label: "Child", description: "Very simple, fun analogies" },
];

function ExplainerConfig({ config, onUpdate, availableInstances, disabled }: ModeConfigProps) {
  // Get current audience levels or use defaults
  const currentLevels = config.audienceLevels || ["expert", "intermediate", "beginner"];

  // Toggle an audience level
  const toggleLevel = (levelId: string) => {
    const current = config.audienceLevels || [];
    const existing = current.indexOf(levelId);

    if (existing !== -1) {
      // Remove if already selected (but don't remove if it's the last one)
      if (current.length > 1) {
        const updated = [...current];
        updated.splice(existing, 1);
        onUpdate({ audienceLevels: updated });
      }
    } else {
      // Add new level
      const updated = [...current, levelId];
      onUpdate({ audienceLevels: updated });
    }
  };

  const isLevelSelected = (levelId: string) => {
    return currentLevels.includes(levelId);
  };

  // Move level up in order
  const moveLevelUp = (index: number) => {
    if (index === 0) return;
    const updated = [...currentLevels];
    [updated[index - 1], updated[index]] = [updated[index], updated[index - 1]];
    onUpdate({ audienceLevels: updated });
  };

  // Move level down in order
  const moveLevelDown = (index: number) => {
    if (index === currentLevels.length - 1) return;
    const updated = [...currentLevels];
    [updated[index], updated[index + 1]] = [updated[index + 1], updated[index]];
    onUpdate({ audienceLevels: updated });
  };

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label className="text-xs">Models</Label>
        <div className="flex flex-wrap gap-1">
          {availableInstances.map((instance, index) => (
            <span
              key={instance.id}
              className="text-[10px] px-1.5 py-0.5 rounded bg-muted text-muted-foreground"
            >
              {index < currentLevels.length
                ? `${currentLevels[index]}: ${getInstanceDisplayName(instance, availableInstances)}`
                : getInstanceDisplayName(instance, availableInstances)}
            </span>
          ))}
        </div>
        <p className="text-[10px] text-muted-foreground">
          Each model generates explanations for different audience levels in order.
          {availableInstances.length < currentLevels.length &&
            ` ${currentLevels.length - availableInstances.length} level(s) will use repeated models.`}
        </p>
      </div>

      <div className="space-y-2">
        <Label className="text-xs">Audience Levels ({currentLevels.length} selected)</Label>
        <div className="space-y-1">
          {DEFAULT_AUDIENCE_LEVEL_OPTIONS.map((option) => (
            <button
              key={option.id}
              type="button"
              onClick={() => toggleLevel(option.id)}
              disabled={disabled}
              className={cn(
                "w-full text-left px-2 py-1.5 rounded text-[11px] transition-colors",
                "border border-transparent",
                isLevelSelected(option.id)
                  ? "bg-primary/10 text-primary border-primary/30"
                  : "bg-muted/50 text-muted-foreground hover:bg-muted hover:text-foreground",
                disabled && "opacity-50 cursor-not-allowed"
              )}
            >
              <div className="flex items-center justify-between">
                <span className="font-medium">{option.label}</span>
                <span className="text-[9px] text-muted-foreground">{option.description}</span>
              </div>
            </button>
          ))}
        </div>
        <p className="text-[10px] text-muted-foreground">
          Select at least 1 audience level. Explanations are generated from most to least complex.
        </p>
      </div>

      {/* Order management */}
      {currentLevels.length > 1 && (
        <div className="space-y-2">
          <Label className="text-xs">Explanation Order</Label>
          <div className="space-y-1">
            {currentLevels.map((levelId, index) => {
              const option = DEFAULT_AUDIENCE_LEVEL_OPTIONS.find((o) => o.id === levelId);
              return (
                <div
                  key={levelId}
                  className="flex items-center gap-2 px-2 py-1 rounded bg-muted/50 text-[11px]"
                >
                  <span className="w-4 text-muted-foreground">{index + 1}.</span>
                  <span className="flex-1 font-medium">{option?.label || levelId}</span>
                  <div className="flex gap-0.5">
                    <button
                      type="button"
                      onClick={() => moveLevelUp(index)}
                      disabled={disabled || index === 0}
                      className="px-1 py-0.5 text-[9px] text-muted-foreground hover:text-foreground disabled:opacity-30"
                    >
                      Up
                    </button>
                    <button
                      type="button"
                      onClick={() => moveLevelDown(index)}
                      disabled={disabled || index === currentLevels.length - 1}
                      className="px-1 py-0.5 text-[9px] text-muted-foreground hover:text-foreground disabled:opacity-30"
                    >
                      Down
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
          <p className="text-[10px] text-muted-foreground">
            First level gets the initial explanation, subsequent levels adapt/simplify it.
          </p>
        </div>
      )}

      <Button
        variant="ghost"
        size="sm"
        onClick={() => onUpdate({ audienceLevels: undefined })}
        disabled={disabled}
        className="text-xs"
      >
        Reset to defaults
      </Button>
    </div>
  );
}

function ConfidenceWeightedConfig({
  config,
  onUpdate,
  availableInstances,
  disabled,
}: ModeConfigProps) {
  const selectedId = getSelectedInstanceId(
    config.synthesizerInstanceId,
    config.synthesizerModel,
    availableInstances
  );
  // Find the synthesizer instance - use selected or default to first
  const synthesizerInstance = selectedId
    ? availableInstances.find((i) => i.id === selectedId)
    : availableInstances[0];
  const respondingInstances = availableInstances.filter((i) => i.id !== synthesizerInstance?.id);

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="synthesizerModel" className="text-xs">
          Synthesizer Model
        </Label>
        <select
          id="synthesizerModel"
          value={selectedId}
          onChange={(e) => {
            const instanceId = e.target.value || undefined;
            const instance = availableInstances.find((i) => i.id === instanceId);
            onUpdate({
              synthesizerInstanceId: instanceId,
              synthesizerModel: instance?.modelId,
            });
          }}
          disabled={disabled}
          className={cn(
            "flex h-9 w-full rounded-md border border-input bg-background px-3 py-1",
            "text-sm ring-offset-background",
            "focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
            "disabled:cursor-not-allowed disabled:opacity-50"
          )}
        >
          <option value="">First selected model (default)</option>
          {availableInstances.map((instance) => (
            <option key={instance.id} value={instance.id}>
              {getInstanceDisplayName(instance, availableInstances)}
            </option>
          ))}
        </select>
        <p className="text-[10px] text-muted-foreground">
          The model that synthesizes responses weighted by confidence scores.
        </p>
      </div>

      <div className="space-y-2">
        <Label className="text-xs">Responding Models</Label>
        {respondingInstances.length === 0 ? (
          <p className="text-[10px] text-muted-foreground italic">
            Select at least 2 models to have responders besides the synthesizer.
          </p>
        ) : (
          <div className="flex flex-wrap gap-1">
            {respondingInstances.map((instance) => (
              <span
                key={instance.id}
                className="text-[10px] px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-700 dark:text-blue-400"
              >
                {getInstanceDisplayName(instance, availableInstances)}
              </span>
            ))}
          </div>
        )}
        <p className="text-[10px] text-muted-foreground">
          These models will respond with self-assessed confidence scores.
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="confidenceThreshold" className="text-xs">
          Minimum Confidence Threshold
        </Label>
        <select
          id="confidenceThreshold"
          value={config.confidenceThreshold ?? 0}
          onChange={(e) => {
            const value = parseFloat(e.target.value);
            onUpdate({ confidenceThreshold: value === 0 ? undefined : value });
          }}
          disabled={disabled}
          className={cn(
            "flex h-9 w-full rounded-md border border-input bg-background px-3 py-1",
            "text-sm ring-offset-background",
            "focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
            "disabled:cursor-not-allowed disabled:opacity-50"
          )}
        >
          <option value={0}>Include all responses (default)</option>
          <option value={0.3}>30% minimum</option>
          <option value={0.5}>50% minimum</option>
          <option value={0.7}>70% minimum</option>
        </select>
        <p className="text-[10px] text-muted-foreground">
          Responses below this confidence level will be excluded from synthesis.
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="confidencePrompt" className="text-xs">
          Confidence Prompt
        </Label>
        <Textarea
          id="confidencePrompt"
          value={config.confidencePrompt || ""}
          onChange={(e) => onUpdate({ confidencePrompt: e.target.value || undefined })}
          disabled={disabled}
          placeholder="Leave empty to use default prompt. Use {question} for the user's question."
          className="min-h-[100px] text-xs"
        />
        <p className="text-[10px] text-muted-foreground">
          Custom instructions for how models should provide confidence scores. Use {"{{question}}"}{" "}
          as a placeholder.
        </p>
      </div>

      <Button
        variant="ghost"
        size="sm"
        onClick={() =>
          onUpdate({
            synthesizerInstanceId: undefined,
            synthesizerModel: undefined,
            confidenceThreshold: undefined,
            confidencePrompt: undefined,
            synthesisPrompt: undefined,
          })
        }
        disabled={disabled}
        className="text-xs"
      >
        Reset to defaults
      </Button>
    </div>
  );
}
