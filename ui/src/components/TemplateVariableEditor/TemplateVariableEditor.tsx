import { ChevronDown, ChevronRight, Plus, Trash2 } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/Button/Button";
import { Input } from "@/components/Input/Input";
import { Switch } from "@/components/Switch/Switch";
import type { TemplateVariable } from "@/lib/templateVariables";

export interface TemplateVariableEditorProps {
  variables: TemplateVariable[];
  onChange: (variables: TemplateVariable[]) => void;
}

const EMPTY_VARIABLE: TemplateVariable = {
  name: "",
  label: "",
  type: "text",
};

export function TemplateVariableEditor({ variables, onChange }: TemplateVariableEditorProps) {
  const [expanded, setExpanded] = useState(variables.length > 0);

  const addVariable = () => {
    onChange([...variables, { ...EMPTY_VARIABLE }]);
    setExpanded(true);
  };

  const removeVariable = (index: number) => {
    onChange(variables.filter((_, i) => i !== index));
  };

  const updateVariable = (index: number, updates: Partial<TemplateVariable>) => {
    onChange(variables.map((v, i) => (i === index ? { ...v, ...updates } : v)));
  };

  return (
    <div className="space-y-2">
      <button
        type="button"
        className="flex items-center gap-1 text-sm font-medium text-foreground hover:text-foreground/80"
        onClick={() => setExpanded(!expanded)}
      >
        {expanded ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
        Variables
        {variables.length > 0 && (
          <span className="ml-1 text-xs text-muted-foreground">({variables.length})</span>
        )}
      </button>

      {expanded && (
        <div className="space-y-3 rounded-md border p-3">
          <p className="text-xs text-muted-foreground">
            Define variables that users fill in when applying this template. Reference them in
            content as {"{{variable_name}}"}.
          </p>

          {variables.map((variable, index) => (
            <div key={index} className="space-y-2 rounded-md border bg-muted/30 p-3">
              <div className="flex items-center justify-between">
                <span className="text-xs font-medium text-muted-foreground">
                  Variable {index + 1}
                </span>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="h-6 w-6 text-muted-foreground hover:text-destructive"
                  onClick={() => removeVariable(index)}
                  aria-label={`Remove variable ${index + 1}`}
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </Button>
              </div>

              <div className="grid grid-cols-2 gap-2">
                <Input
                  placeholder="name (e.g. language)"
                  value={variable.name}
                  onChange={(e) =>
                    updateVariable(index, {
                      name: e.target.value.replace(/[^a-zA-Z0-9_]/g, ""),
                    })
                  }
                />
                <Input
                  placeholder="Label (e.g. Language)"
                  value={variable.label}
                  onChange={(e) => updateVariable(index, { label: e.target.value })}
                />
              </div>

              <div className="grid grid-cols-2 gap-2">
                <select
                  value={variable.type}
                  onChange={(e) =>
                    updateVariable(index, {
                      type: e.target.value as TemplateVariable["type"],
                    })
                  }
                  className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                  aria-label="Variable type"
                >
                  <option value="text">Text</option>
                  <option value="textarea">Textarea</option>
                  <option value="select">Select</option>
                </select>
                <Input
                  placeholder="Default value"
                  value={variable.default ?? ""}
                  onChange={(e) => updateVariable(index, { default: e.target.value || undefined })}
                />
              </div>

              <div className="flex items-center gap-4">
                <Input
                  placeholder="Placeholder text"
                  value={variable.placeholder ?? ""}
                  onChange={(e) =>
                    updateVariable(index, { placeholder: e.target.value || undefined })
                  }
                  className="flex-1"
                />
                <span className="flex items-center gap-2 text-sm whitespace-nowrap">
                  <Switch
                    checked={variable.required ?? false}
                    onChange={(e) => updateVariable(index, { required: e.target.checked })}
                    aria-label="Required"
                  />
                  Required
                </span>
              </div>

              {variable.type === "select" && (
                <Input
                  placeholder="Options (comma-separated)"
                  value={variable.options?.join(", ") ?? ""}
                  onChange={(e) =>
                    updateVariable(index, {
                      options: e.target.value
                        .split(",")
                        .map((s) => s.trim())
                        .filter(Boolean),
                    })
                  }
                />
              )}
            </div>
          ))}

          <Button type="button" variant="outline" size="sm" onClick={addVariable}>
            <Plus className="mr-1 h-3.5 w-3.5" />
            Add Variable
          </Button>
        </div>
      )}
    </div>
  );
}
