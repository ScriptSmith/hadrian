import { useId } from "react";

import type { Team } from "@/api/generated/types.gen";

export interface TeamSelectProps {
  teams: Team[];
  value: string | null;
  onChange: (teamId: string | null) => void;
  label?: string;
  className?: string;
  /** Allow selecting "None" to clear team assignment */
  allowNone?: boolean;
  /** Placeholder text for the "None" option */
  nonePlaceholder?: string;
  /** Whether the field is disabled */
  disabled?: boolean;
}

export function TeamSelect({
  teams,
  value,
  onChange,
  label = "Team",
  className,
  allowNone = true,
  nonePlaceholder = "None (Organization-level)",
  disabled = false,
}: TeamSelectProps) {
  const selectId = useId();

  return (
    <div className={className}>
      {label && (
        <label htmlFor={selectId} className="mb-1 block text-sm font-medium">
          {label}
        </label>
      )}
      <select
        id={selectId}
        value={value || ""}
        onChange={(e) => onChange(e.target.value || null)}
        disabled={disabled}
        className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm disabled:cursor-not-allowed disabled:opacity-50"
      >
        {allowNone && <option value="">{nonePlaceholder}</option>}
        {teams.map((team) => (
          <option key={team.id} value={team.id}>
            {team.name}
          </option>
        ))}
      </select>
    </div>
  );
}
