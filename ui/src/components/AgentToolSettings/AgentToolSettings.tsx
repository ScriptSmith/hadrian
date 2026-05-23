/**
 * AgentToolSettings — container configuration for the "Agent (Shell)" tool.
 *
 * Rendered as the tool's settings flyout in the ToolsBar. Enabling/disabling
 * the tool is handled by the ToolsBar toggle (the `agent` tool id); this panel
 * only configures the container. Changing any setting auto-enables the tool via
 * `onConfigured`, mirroring how picking a sub-agent model enables `sub_agent`.
 */

import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";

import { apiV1ContainersListOptions } from "@/api/generated/@tanstack/react-query.gen";
import { Input } from "@/components/Input/Input";
import { Textarea } from "@/components/Textarea/Textarea";
import { NumberInput } from "@/components/NumberInput/NumberInput";
import { Select } from "@/components/Select/Select";
import { FormField } from "@/components/FormField/FormField";
import { cn } from "@/utils/cn";
import {
  useChatUIStore,
  useAgentContainerMode,
  useAgentContainerId,
  useAgentMemoryLimit,
  useAgentExpiresAfterMinutes,
  useAgentAllowedDomains,
} from "@/stores/chatUIStore";
import type { ContainerList } from "@/pages/containers/types";

export interface AgentToolSettingsProps {
  disabled?: boolean;
  /** Called when the user configures the agent, so the tool is enabled. */
  onConfigured?: () => void;
}

export function AgentToolSettings({ disabled = false, onConfigured }: AgentToolSettingsProps) {
  const containerMode = useAgentContainerMode();
  const containerId = useAgentContainerId();
  const memoryLimit = useAgentMemoryLimit();
  const expiresAfterMinutes = useAgentExpiresAfterMinutes();
  const allowedDomains = useAgentAllowedDomains();

  const setContainerMode = useChatUIStore((s) => s.setAgentContainerMode);
  const setContainerId = useChatUIStore((s) => s.setAgentContainerId);
  const setMemoryLimit = useChatUIStore((s) => s.setAgentMemoryLimit);
  const setExpiresAfterMinutes = useChatUIStore((s) => s.setAgentExpiresAfterMinutes);
  const setAllowedDomains = useChatUIStore((s) => s.setAgentAllowedDomains);

  const { data: containersData } = useQuery({
    ...apiV1ContainersListOptions({ query: { limit: 100 } }),
    enabled: containerMode === "reference",
    staleTime: 30 * 1000,
  });

  const containerOptions = useMemo(() => {
    const list = (containersData as ContainerList | undefined)?.data ?? [];
    return list
      .filter((c) => c.status === "active")
      .map((c) => ({ value: c.id, label: c.name?.trim() || c.id }));
  }, [containersData]);

  const touch = () => onConfigured?.();

  return (
    <div className="w-72 space-y-3 p-1">
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
              disabled={disabled}
              onClick={() => {
                setContainerMode(opt.value);
                touch();
              }}
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
            onChange={(v) => {
              setContainerId(v);
              touch();
            }}
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
              disabled={disabled}
              onChange={(e) => {
                setMemoryLimit(e.target.value);
                touch();
              }}
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
              onChange={(v) => {
                setExpiresAfterMinutes(v > 0 ? v : null);
                touch();
              }}
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
              disabled={disabled}
              onChange={(e) => {
                setAllowedDomains(e.target.value);
                touch();
              }}
              placeholder="*"
              rows={2}
            />
          </FormField>
        </>
      )}
    </div>
  );
}
