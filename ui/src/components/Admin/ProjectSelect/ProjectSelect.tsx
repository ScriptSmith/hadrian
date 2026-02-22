import { useId } from "react";

import type { Project } from "@/api/generated/types.gen";

export interface ProjectSelectProps {
  projects: Project[];
  value: string | null;
  onChange: (slug: string | null) => void;
  label?: string;
  className?: string;
  /** Allow selecting "None" to clear project assignment */
  allowNone?: boolean;
  /** Placeholder text for the "None" option */
  nonePlaceholder?: string;
  /** Whether the field is disabled */
  disabled?: boolean;
}

export function ProjectSelect({
  projects,
  value,
  onChange,
  label = "Project",
  className,
  allowNone = true,
  nonePlaceholder = "All projects",
  disabled = false,
}: ProjectSelectProps) {
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
        {projects.map((project) => (
          <option key={project.id} value={project.slug}>
            {project.name}
          </option>
        ))}
      </select>
    </div>
  );
}
