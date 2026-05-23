/**
 * AgentButton — turns a conversation into an agent run.
 *
 * Sits beside Skills/Templates in the chat input. Toggling it on attaches the
 * shell tool (with a fresh or reused container) to the `/v1/responses`
 * request, and optionally a `tool_search` tool so the model lazily discovers
 * tools. All state is session-scoped in `chatUIStore`.
 */

import { useMemo } from "react";
import { Bot } from "lucide-react";
import { useQuery } from "@tanstack/react-query";

import { apiV1ContainersListOptions } from "@/api/generated/@tanstack/react-query.gen";
import { Button } from "@/components/Button/Button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/Popover/Popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { Switch } from "@/components/Switch/Switch";
import { Input } from "@/components/Input/Input";
import { Textarea } from "@/components/Textarea/Textarea";
import { NumberInput } from "@/components/NumberInput/NumberInput";
import { Select } from "@/components/Select/Select";
import { FormField } from "@/components/FormField/FormField";
import { cn } from "@/utils/cn";
import {
  useChatUIStore,
  useAgentEnabled,
  useAgentContainerMode,
  useAgentContainerId,
  useAgentMemoryLimit,
  useAgentExpiresAfterMinutes,
  useAgentAllowedDomains,
  useToolSearchEnabled,
  useToolSearchRanker,
} from "@/stores/chatUIStore";
import type { ContainerList } from "@/pages/containers/types";

export interface AgentButtonProps {
  disabled?: boolean;
}

export function AgentButton({ disabled = false }: AgentButtonProps) {
  const agentEnabled = useAgentEnabled();
  const containerMode = useAgentContainerMode();
  const containerId = useAgentContainerId();
  const memoryLimit = useAgentMemoryLimit();
  const expiresAfterMinutes = useAgentExpiresAfterMinutes();
  const allowedDomains = useAgentAllowedDomains();
  const toolSearchEnabled = useToolSearchEnabled();
  const toolSearchRanker = useToolSearchRanker();

  const setAgentEnabled = useChatUIStore((s) => s.setAgentEnabled);
  const setContainerMode = useChatUIStore((s) => s.setAgentContainerMode);
  const setContainerId = useChatUIStore((s) => s.setAgentContainerId);
  const setMemoryLimit = useChatUIStore((s) => s.setAgentMemoryLimit);
  const setExpiresAfterMinutes = useChatUIStore((s) => s.setAgentExpiresAfterMinutes);
  const setAllowedDomains = useChatUIStore((s) => s.setAgentAllowedDomains);
  const setToolSearchEnabled = useChatUIStore((s) => s.setToolSearchEnabled);
  const setToolSearchRanker = useChatUIStore((s) => s.setToolSearchRanker);

  // Active containers for the "attach existing" picker. Only fetched when the
  // popover is likely needed (agent enabled + reference mode), but harmless to
  // keep enabled — it's a small, cached list.
  const { data: containersData } = useQuery({
    ...apiV1ContainersListOptions({ query: { limit: 100 } }),
    enabled: agentEnabled && containerMode === "reference",
    staleTime: 30 * 1000,
  });

  const containerOptions = useMemo(() => {
    const list = (containersData as ContainerList | undefined)?.data ?? [];
    return list
      .filter((c) => c.status === "active")
      .map((c) => ({ value: c.id, label: c.name?.trim() || c.id }));
  }, [containersData]);

  return (
    <Popover>
      <Tooltip>
        <TooltipTrigger asChild>
          <PopoverTrigger asChild>
            <Button
              type="button"
              size="icon"
              variant="ghost"
              disabled={disabled}
              aria-label="Agent tools"
              aria-pressed={agentEnabled}
              className={cn(
                "h-8 w-8 shrink-0 rounded-lg",
                agentEnabled ? "text-primary" : "text-muted-foreground hover:text-foreground"
              )}
            >
              <Bot className="h-4 w-4" />
            </Button>
          </PopoverTrigger>
        </TooltipTrigger>
        <TooltipContent side="top">Agent tools</TooltipContent>
      </Tooltip>

      <PopoverContent className="w-80 space-y-3 p-3" align="start">
        <div className="flex items-center justify-between gap-2">
          <div>
            <p className="text-sm font-medium">Shell tool</p>
            <p className="text-xs text-muted-foreground">
              Let the model run commands in a container.
            </p>
          </div>
          <Switch
            checked={agentEnabled}
            onChange={(e) => setAgentEnabled(e.target.checked)}
            aria-label="Enable shell tool"
          />
        </div>

        {agentEnabled && (
          <div className="space-y-3 border-t border-border pt-3">
            {/* Container mode */}
            <FormField label="Container" htmlFor="agent-container-mode">
              <div className="flex gap-1.5" role="radiogroup" aria-label="Container mode">
                {[
                  { value: "auto" as const, label: "New" },
                  { value: "reference" as const, label: "Attach existing" },
                ].map((opt) => (
                  <button
                    key={opt.value}
                    type="button"
                    role="radio"
                    aria-checked={containerMode === opt.value}
                    onClick={() => setContainerMode(opt.value)}
                    className={cn(
                      "px-3 py-1.5 rounded-md text-sm border transition-colors",
                      containerMode === opt.value
                        ? "border-primary bg-primary/10 text-primary font-medium"
                        : "border-input text-muted-foreground hover:bg-muted"
                    )}
                  >
                    {opt.label}
                  </button>
                ))}
              </div>
            </FormField>

            {containerMode === "reference" ? (
              <FormField
                label="Existing container"
                htmlFor="agent-container-id"
                helpText={
                  containerOptions.length === 0
                    ? "No active containers. Run an agent turn to create one."
                    : undefined
                }
              >
                <Select
                  value={containerId}
                  onChange={(v) => setContainerId(v)}
                  options={containerOptions}
                  placeholder="Select a container..."
                />
              </FormField>
            ) : (
              <>
                <FormField
                  label="Memory limit"
                  htmlFor="agent-memory-limit"
                  helpText="Optional, e.g. 512m or 1g. Blank uses the default."
                >
                  <Input
                    id="agent-memory-limit"
                    value={memoryLimit}
                    onChange={(e) => setMemoryLimit(e.target.value)}
                    placeholder="default"
                  />
                </FormField>

                <FormField
                  label="Idle timeout (minutes)"
                  htmlFor="agent-idle-ttl"
                  helpText="Optional. Container is reaped after this long without activity."
                >
                  <NumberInput
                    id="agent-idle-ttl"
                    value={expiresAfterMinutes ?? 0}
                    onChange={(v) => setExpiresAfterMinutes(v > 0 ? v : null)}
                    min={0}
                    max={1440}
                  />
                </FormField>

                <FormField
                  label="Allowed egress domains"
                  htmlFor="agent-allowed-domains"
                  helpText="Comma-separated. `*` allows any host (capped by the operator); blank means deny-all."
                >
                  <Textarea
                    id="agent-allowed-domains"
                    value={allowedDomains}
                    onChange={(e) => setAllowedDomains(e.target.value)}
                    placeholder="pypi.org, files.pythonhosted.org"
                    rows={2}
                  />
                </FormField>
              </>
            )}
          </div>
        )}

        {/* Tool search (applies to agent + MCP tools) */}
        <div className="flex items-center justify-between gap-2 border-t border-border pt-3">
          <div>
            <p className="text-sm font-medium">Tool search</p>
            <p className="text-xs text-muted-foreground">
              Let the model search for tools and load them on demand.
            </p>
          </div>
          <Switch
            checked={toolSearchEnabled}
            onChange={(e) => setToolSearchEnabled(e.target.checked)}
            aria-label="Enable tool search"
          />
        </div>

        {toolSearchEnabled && (
          <FormField
            label="Ranker"
            htmlFor="agent-tool-search-ranker"
            helpText="Semantic and hybrid need an embedding provider; the gateway falls back with an error otherwise."
          >
            <Select
              value={toolSearchRanker}
              onChange={(v) => setToolSearchRanker((v ?? "default") as typeof toolSearchRanker)}
              options={[
                { value: "default", label: "Default" },
                { value: "hybrid", label: "Hybrid" },
                { value: "semantic", label: "Semantic" },
                { value: "lexical", label: "Lexical" },
              ]}
            />
          </FormField>
        )}
      </PopoverContent>
    </Popover>
  );
}
