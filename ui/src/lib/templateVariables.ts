export interface TemplateVariable {
  /** {{name}} reference in content */
  name: string;
  /** Display label */
  label: string;
  /** Field type */
  type: "text" | "textarea" | "select";
  /** Whether the variable is required */
  required?: boolean;
  /** Placeholder text */
  placeholder?: string;
  /** Default value */
  default?: string;
  /** Options for select type */
  options?: string[];
}

/** Parse variables from prompt metadata */
export function parseVariables(metadata?: Record<string, unknown> | null): TemplateVariable[] {
  if (!metadata?.variables || !Array.isArray(metadata.variables)) return [];
  return metadata.variables.filter(
    (v: unknown): v is TemplateVariable =>
      typeof v === "object" && v !== null && "name" in v && "label" in v && "type" in v
  );
}

/** Extract {{var}} references from content string */
export function extractVariableRefs(content: string): string[] {
  const matches = content.match(/\{\{(\w+)\}\}/g);
  if (!matches) return [];
  return [...new Set(matches.map((m) => m.slice(2, -2)))];
}

/** Replace {{var}} placeholders with provided values */
export function substituteVariables(content: string, values: Record<string, string>): string {
  return content.replace(/\{\{(\w+)\}\}/g, (match, name: string) => values[name] ?? match);
}

/** Validate variable values, return map of field name -> error message */
export function validateVariableValues(
  variables: TemplateVariable[],
  values: Record<string, string>
): Record<string, string> {
  const errors: Record<string, string> = {};
  for (const v of variables) {
    const val = values[v.name]?.trim() ?? "";
    if (v.required && !val) {
      errors[v.name] = `${v.label} is required`;
    }
  }
  return errors;
}
