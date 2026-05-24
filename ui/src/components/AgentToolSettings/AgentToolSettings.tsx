/**
 * AgentToolSettings — container configuration for the "Agent (Shell)" tool.
 *
 * Rendered as the tool's settings flyout in the ToolsBar. Enabling/disabling
 * the tool is handled by the ToolsBar toggle (the `agent` tool id); this panel
 * configures the container a *new* conversation provisions. The conversation
 * then reuses that container until it expires (handled in useChat), so there's
 * no manual container picker. Changing a setting auto-enables the tool via
 * `onConfigured`, mirroring how picking a sub-agent model enables `sub_agent`.
 */

import { Input } from "@/components/Input/Input";
import { Textarea } from "@/components/Textarea/Textarea";
import { NumberInput } from "@/components/NumberInput/NumberInput";
import { FormField } from "@/components/FormField/FormField";
import {
  useChatUIStore,
  useAgentMemoryLimit,
  useAgentExpiresAfterMinutes,
  useAgentAllowedDomains,
} from "@/stores/chatUIStore";

export interface AgentToolSettingsProps {
  disabled?: boolean;
  /** Called when the user configures the agent, so the tool is enabled. */
  onConfigured?: () => void;
}

export function AgentToolSettings({ disabled = false, onConfigured }: AgentToolSettingsProps) {
  const memoryLimit = useAgentMemoryLimit();
  const expiresAfterMinutes = useAgentExpiresAfterMinutes();
  const allowedDomains = useAgentAllowedDomains();

  const setMemoryLimit = useChatUIStore((s) => s.setAgentMemoryLimit);
  const setExpiresAfterMinutes = useChatUIStore((s) => s.setAgentExpiresAfterMinutes);
  const setAllowedDomains = useChatUIStore((s) => s.setAgentAllowedDomains);

  const touch = () => onConfigured?.();

  return (
    <div className="w-full space-y-3">
      <p className="text-xs text-muted-foreground">
        Runs shell commands in a container that persists for the whole conversation, until it
        expires.
      </p>

      <FormField
        label="Memory limit"
        htmlFor="agent-memory-limit"
        helpText="Optional, e.g. 512m or 1g. Blank uses the default."
      >
        <Input
          id="agent-memory-limit"
          className="w-full"
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
          className="w-full"
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
          className="w-full"
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
    </div>
  );
}
