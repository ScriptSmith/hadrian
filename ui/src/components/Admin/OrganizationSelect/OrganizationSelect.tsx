import { useId } from "react";

import type { Organization } from "@/api/generated/types.gen";

export interface OrganizationSelectProps {
  organizations: Organization[];
  value: string | null;
  onChange: (slug: string | null) => void;
  label?: string;
  className?: string;
}

export function OrganizationSelect({
  organizations,
  value,
  onChange,
  label = "Filter by Organization",
  className,
}: OrganizationSelectProps) {
  const selectId = useId();

  if (organizations.length <= 1) {
    return null;
  }

  return (
    <div className={className}>
      <label htmlFor={selectId} className="mb-1 block text-sm font-medium">
        {label}
      </label>
      <select
        id={selectId}
        value={value || organizations[0]?.slug || ""}
        onChange={(e) => onChange(e.target.value || null)}
        className="rounded-md border border-input bg-background px-3 py-2 text-sm"
      >
        {organizations.map((org) => (
          <option key={org.slug} value={org.slug}>
            {org.name}
          </option>
        ))}
      </select>
    </div>
  );
}
