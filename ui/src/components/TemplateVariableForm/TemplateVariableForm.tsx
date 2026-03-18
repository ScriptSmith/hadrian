import { FormField } from "@/components/FormField/FormField";
import { Input } from "@/components/Input/Input";
import type { TemplateVariable } from "@/lib/templateVariables";

export interface TemplateVariableFormProps {
  variables: TemplateVariable[];
  values: Record<string, string>;
  onChange: (values: Record<string, string>) => void;
  errors?: Record<string, string>;
}

export function TemplateVariableForm({
  variables,
  values,
  onChange,
  errors,
}: TemplateVariableFormProps) {
  const handleChange = (name: string, value: string) => {
    onChange({ ...values, [name]: value });
  };

  return (
    <div className="space-y-3">
      {variables.map((v) => (
        <FormField
          key={v.name}
          label={v.label}
          htmlFor={`tplvar-${v.name}`}
          required={v.required}
          error={errors?.[v.name]}
        >
          {v.type === "select" ? (
            <select
              id={`tplvar-${v.name}`}
              value={values[v.name] ?? v.default ?? ""}
              onChange={(e) => handleChange(v.name, e.target.value)}
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            >
              <option value="">{v.placeholder ?? "Select..."}</option>
              {v.options?.map((opt) => (
                <option key={opt} value={opt}>
                  {opt}
                </option>
              ))}
            </select>
          ) : v.type === "textarea" ? (
            <textarea
              id={`tplvar-${v.name}`}
              value={values[v.name] ?? v.default ?? ""}
              onChange={(e) => handleChange(v.name, e.target.value)}
              placeholder={v.placeholder}
              className="w-full min-h-[80px] rounded-md border bg-background px-3 py-2 text-sm placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 resize-y"
            />
          ) : (
            <Input
              id={`tplvar-${v.name}`}
              value={values[v.name] ?? v.default ?? ""}
              onChange={(e) => handleChange(v.name, e.target.value)}
              placeholder={v.placeholder}
            />
          )}
        </FormField>
      ))}
    </div>
  );
}
