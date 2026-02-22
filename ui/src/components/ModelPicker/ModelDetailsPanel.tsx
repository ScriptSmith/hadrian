import {
  Brain,
  Wrench,
  Eye,
  Braces,
  Scale,
  Calendar,
  Hash,
  Layers,
  Clock,
  DollarSign,
  MessageSquare,
  Sparkles,
  Cpu,
  X,
  Info,
} from "lucide-react";

import { cn } from "@/utils/cn";

import { CapabilityBadge } from "./CapabilityBadge";
import type { ModelInfo } from "./model-utils";
import {
  getModelName,
  getProviderFromId,
  getProviderInfo,
  formatContextLength,
  formatMaxOutputTokens,
  formatDate,
  formatCatalogPricing,
  getModelType,
} from "./model-utils";

interface ModelDetailsPanelProps {
  model: ModelInfo | null;
  className?: string;
  /** Called when user clicks the close button */
  onClose?: () => void;
}

/**
 * Side panel displaying detailed information about a model.
 * Shows when user clicks the info button on a model card.
 */
export function ModelDetailsPanel({ model, className, onClose }: ModelDetailsPanelProps) {
  if (!model) {
    return (
      <div
        className={cn(
          "flex flex-col items-center justify-center text-muted-foreground text-sm gap-3",
          className
        )}
      >
        <Info className="h-8 w-8 text-muted-foreground/50" />
        <p className="text-center px-4">Click the info icon on a model to see details</p>
      </div>
    );
  }

  const provider = getProviderFromId(model.id);
  const providerInfo = getProviderInfo(provider, model.source);
  const modelType = getModelType(model.id, model.capabilities, model.modalities);
  const capabilities = model.capabilities;
  const catalogPricing = model.catalog_pricing;

  return (
    <div className={cn("flex flex-col p-4 overflow-y-auto", className)}>
      {/* Header */}
      <div className="space-y-2 pb-4 border-b border-border">
        <div className="flex items-start justify-between gap-2">
          <h3 className="font-semibold text-base leading-tight">{getModelName(model.id)}</h3>
          {onClose && (
            <button
              type="button"
              onClick={onClose}
              className="shrink-0 rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
              aria-label="Close details"
            >
              <X className="h-4 w-4" />
            </button>
          )}
        </div>
        <p className="text-xs text-muted-foreground font-mono break-all">{model.id}</p>
        <div className="flex items-center gap-2 flex-wrap">
          <span className={cn("rounded px-2 py-0.5 text-xs font-medium", providerInfo.color)}>
            {providerInfo.label}
          </span>
          {model.source === "dynamic" && (
            <span className="rounded px-1.5 py-0.5 text-[10px] font-medium bg-emerald-500/10 text-emerald-700 dark:text-emerald-400 border border-emerald-500/20">
              My Provider
            </span>
          )}
          <span className="flex items-center gap-1 text-xs text-muted-foreground">
            {modelType.icon === "sparkles" ? (
              <Sparkles className="h-3.5 w-3.5" />
            ) : (
              <Cpu className="h-3.5 w-3.5" />
            )}
            {modelType.label}
          </span>
        </div>
      </div>

      {/* Capabilities */}
      {capabilities && (
        <div className="py-4 border-b border-border">
          <h4 className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-3">
            Capabilities
          </h4>
          <div className="flex flex-wrap gap-2">
            {capabilities.reasoning && (
              <div className="flex items-center gap-1.5 text-sm">
                <CapabilityBadge icon={Brain} label="Reasoning" color="purple" />
                <span>Reasoning</span>
              </div>
            )}
            {capabilities.tool_call && (
              <div className="flex items-center gap-1.5 text-sm">
                <CapabilityBadge icon={Wrench} label="Tool Calling" color="green" />
                <span>Tools</span>
              </div>
            )}
            {capabilities.vision && (
              <div className="flex items-center gap-1.5 text-sm">
                <CapabilityBadge icon={Eye} label="Vision" color="cyan" />
                <span>Vision</span>
              </div>
            )}
            {capabilities.structured_output && (
              <div className="flex items-center gap-1.5 text-sm">
                <CapabilityBadge icon={Braces} label="Structured Output" color="amber" />
                <span>JSON</span>
              </div>
            )}
            {model.open_weights && (
              <div className="flex items-center gap-1.5 text-sm">
                <CapabilityBadge icon={Scale} label="Open Weights" color="indigo" />
                <span>Open</span>
              </div>
            )}
            {!capabilities.reasoning &&
              !capabilities.tool_call &&
              !capabilities.vision &&
              !capabilities.structured_output &&
              !model.open_weights && (
                <span className="text-sm text-muted-foreground">No special capabilities</span>
              )}
          </div>
        </div>
      )}

      {/* Specifications */}
      <div className="py-4 border-b border-border">
        <h4 className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-3">
          Specifications
        </h4>
        <div className="space-y-2.5">
          {model.context_length && (
            <DetailRow
              icon={MessageSquare}
              label="Context window"
              value={`${formatContextLength(model.context_length)} tokens`}
            />
          )}
          {model.max_output_tokens && (
            <DetailRow
              icon={Hash}
              label="Max output"
              value={`${formatMaxOutputTokens(model.max_output_tokens)} tokens`}
            />
          )}
          {model.family && <DetailRow icon={Layers} label="Model family" value={model.family} />}
          {model.knowledge_cutoff && (
            <DetailRow
              icon={Calendar}
              label="Knowledge cutoff"
              value={formatDate(model.knowledge_cutoff)}
            />
          )}
          {model.release_date && (
            <DetailRow icon={Clock} label="Released" value={formatDate(model.release_date)} />
          )}
        </div>
      </div>

      {/* Pricing */}
      {catalogPricing && (
        <div className="py-4 border-b border-border">
          <h4 className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-3">
            Pricing
          </h4>
          <div className="space-y-2.5">
            <DetailRow
              icon={DollarSign}
              label="Input"
              value={
                catalogPricing.input === 0 ? (
                  <span className="text-green-700 dark:text-green-400">Free</span>
                ) : (
                  `${formatCatalogPricing(catalogPricing.input)} / 1M tokens`
                )
              }
            />
            <DetailRow
              icon={DollarSign}
              label="Output"
              value={
                catalogPricing.output === 0 ? (
                  <span className="text-green-700 dark:text-green-400">Free</span>
                ) : (
                  `${formatCatalogPricing(catalogPricing.output)} / 1M tokens`
                )
              }
            />
            {catalogPricing.reasoning !== undefined && catalogPricing.reasoning > 0 && (
              <DetailRow
                icon={Brain}
                label="Reasoning"
                value={`${formatCatalogPricing(catalogPricing.reasoning)} / 1M tokens`}
              />
            )}
          </div>
        </div>
      )}

      {/* Description */}
      {model.description && (
        <div className="pt-4">
          <h4 className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-2">
            Description
          </h4>
          <p className="text-sm text-muted-foreground leading-relaxed">{model.description}</p>
        </div>
      )}
    </div>
  );
}

function DetailRow({
  icon: Icon,
  label,
  value,
}: {
  icon: typeof Brain;
  label: string;
  value: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-2 text-sm">
      <span className="flex items-center gap-2 text-muted-foreground">
        <Icon className="h-4 w-4" />
        {label}
      </span>
      <span className="font-medium text-right">{value}</span>
    </div>
  );
}
