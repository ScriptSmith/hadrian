import { useEffect, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { CheckCircle2, XCircle, Loader2, Info } from "lucide-react";
import { useDebouncedCallback } from "use-debounce";

import { orgRbacPolicyValidateMutation } from "@/api/generated/@tanstack/react-query.gen";
import { Textarea } from "@/components/Textarea/Textarea";
import { cn } from "@/utils/cn";

interface CelExpressionInputProps {
  value: string;
  onChange: (value: string) => void;
  error?: string;
  disabled?: boolean;
  placeholder?: string;
}

const CEL_HELP_TEXT = `Available variables:
  subject.roles        - User's roles (array)
  subject.user_id      - User ID
  subject.email        - User email
  subject.external_id  - External IdP ID
  subject.org_ids      - Organizations (array)
  subject.team_ids     - Teams (array)
  subject.project_ids  - Projects (array)
  context.resource_type - Resource being accessed
  context.action       - Action being performed
  context.resource_id  - Specific resource ID
  context.org_id       - Organization context
  context.team_id      - Team context
  context.project_id   - Project context

Examples:
  'admin' in subject.roles
  subject.email.endsWith('@acme.com')
  context.action == 'read' || 'viewer' in subject.roles
  context.org_id in subject.org_ids`;

export function CelExpressionInput({
  value,
  onChange,
  error: externalError,
  disabled,
  placeholder = "'admin' in subject.roles",
}: CelExpressionInputProps) {
  const [validationState, setValidationState] = useState<{
    valid: boolean | null;
    error: string | null;
    checking: boolean;
  }>({ valid: null, error: null, checking: false });

  const [showHelp, setShowHelp] = useState(false);

  const validateMutation = useMutation({
    ...orgRbacPolicyValidateMutation(),
    onSuccess: (data) => {
      setValidationState({
        valid: data.valid,
        error: data.error ?? null,
        checking: false,
      });
    },
    onError: () => {
      setValidationState({
        valid: null,
        error: "Failed to validate expression",
        checking: false,
      });
    },
  });

  const debouncedValidate = useDebouncedCallback((condition: string) => {
    if (!condition.trim()) {
      setValidationState({ valid: null, error: null, checking: false });
      return;
    }
    setValidationState((prev) => ({ ...prev, checking: true }));
    validateMutation.mutate({ body: { condition } });
  }, 500);

  useEffect(() => {
    if (value) {
      setValidationState((prev) => ({ ...prev, checking: true }));
      debouncedValidate(value);
    } else {
      setValidationState({ valid: null, error: null, checking: false });
    }
  }, [value, debouncedValidate]);

  const hasError = externalError || validationState.error;
  const isValid = validationState.valid === true && !externalError;

  return (
    <div className="space-y-2">
      <div className="relative">
        <Textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          disabled={disabled}
          placeholder={placeholder}
          aria-label="CEL condition expression"
          rows={3}
          className={cn(
            "font-mono text-sm pr-10",
            isValid && "border-green-500 focus-visible:ring-green-500",
            hasError && "border-destructive focus-visible:ring-destructive"
          )}
        />
        <div className="absolute right-3 top-3">
          {validationState.checking ? (
            <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
          ) : isValid ? (
            <CheckCircle2 className="h-4 w-4 text-green-500" />
          ) : hasError ? (
            <XCircle className="h-4 w-4 text-destructive" />
          ) : null}
        </div>
      </div>

      {hasError && (
        <p className="text-sm text-destructive">{externalError || validationState.error}</p>
      )}

      <div className="flex items-start gap-2">
        <button
          type="button"
          onClick={() => setShowHelp(!showHelp)}
          className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          <Info className="h-3 w-3" />
          {showHelp ? "Hide help" : "Show CEL syntax help"}
        </button>
      </div>

      {showHelp && (
        <pre className="text-xs text-muted-foreground bg-muted p-3 rounded-md overflow-x-auto whitespace-pre-wrap">
          {CEL_HELP_TEXT}
        </pre>
      )}
    </div>
  );
}
