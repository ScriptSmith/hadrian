import { useId } from "react";

import type { User } from "@/api/generated/types.gen";

export interface UserSelectProps {
  users: User[];
  value: string | null;
  onChange: (userId: string | null) => void;
  label?: string;
  className?: string;
  /** Allow selecting "None" to clear user assignment */
  allowNone?: boolean;
  /** Placeholder text for the "None" option */
  nonePlaceholder?: string;
  /** Whether the field is disabled */
  disabled?: boolean;
}

function displayName(user: User): string {
  return user.name || user.email || user.external_id;
}

export function UserSelect({
  users,
  value,
  onChange,
  label = "User",
  className,
  allowNone = true,
  nonePlaceholder = "All users",
  disabled = false,
}: UserSelectProps) {
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
        {users.map((user) => (
          <option key={user.id} value={user.id}>
            {displayName(user)}
          </option>
        ))}
      </select>
    </div>
  );
}
