import { Info } from "lucide-react";
import type { UseFormRegister } from "react-hook-form";
import { z } from "zod";

import { FormField } from "@/components/FormField/FormField";
import { Input } from "@/components/Input/Input";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";

/** Zod schema fragment for sovereignty requirement fields. */
export const sovereigntySchema = {
  sov_inference_countries: z.string().optional(),
  sov_blocked_countries: z.string().optional(),
  sov_certifications: z.string().optional(),
  sov_licenses: z.string().optional(),
  sov_require_on_prem: z.boolean().optional(),
  sov_require_open_weights: z.boolean().optional(),
};

/** Default form values for sovereignty fields. */
export const sovereigntyDefaults = {
  sov_inference_countries: "",
  sov_blocked_countries: "",
  sov_certifications: "",
  sov_licenses: "",
  sov_require_on_prem: false,
  sov_require_open_weights: false,
};

/** Parse a comma-separated string into a trimmed, non-empty array. */
export function parseCsv(value: string | undefined): string[] {
  if (!value || value.trim() === "") return [];
  return value
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean);
}

/**
 * Build the sovereignty_requirements object from form data, or undefined if
 * no sovereignty constraints are set.
 */
export function buildSovereigntyRequirements(data: {
  sov_inference_countries?: string;
  sov_blocked_countries?: string;
  sov_certifications?: string;
  sov_licenses?: string;
  sov_require_on_prem?: boolean;
  sov_require_open_weights?: boolean;
}): Record<string, unknown> | undefined {
  const sovInference = parseCsv(data.sov_inference_countries);
  const sovBlocked = parseCsv(data.sov_blocked_countries);
  const sovCerts = parseCsv(data.sov_certifications);
  const sovLicenses = parseCsv(data.sov_licenses);
  const hasSovereignty =
    sovInference.length > 0 ||
    sovBlocked.length > 0 ||
    sovCerts.length > 0 ||
    sovLicenses.length > 0 ||
    data.sov_require_on_prem ||
    data.sov_require_open_weights;

  if (!hasSovereignty) return undefined;

  return {
    ...(sovInference.length > 0 && { allowed_inference_countries: sovInference }),
    ...(data.sov_require_on_prem && { require_on_prem: true }),
    ...(sovCerts.length > 0 && { required_certifications: sovCerts }),
    ...(data.sov_require_open_weights && { require_open_weights: true }),
    ...(sovBlocked.length > 0 && { blocked_hq_countries: sovBlocked }),
    ...(sovLicenses.length > 0 && { allowed_licenses: sovLicenses }),
  };
}

/** Form fields that include the sovereignty schema shape. */
type SovereigntyFormShape = {
  sov_inference_countries?: string;
  sov_blocked_countries?: string;
  sov_certifications?: string;
  sov_licenses?: string;
  sov_require_on_prem?: boolean;
  sov_require_open_weights?: boolean;
};

interface SovereigntyFormFieldsProps<T extends SovereigntyFormShape> {
  register: UseFormRegister<T>;
  /** Optional id prefix for HTML ids (e.g. "apikey" or "self-apikey"). */
  idPrefix?: string;
}

/**
 * Shared sovereignty requirement form fields for API key creation modals.
 * Renders the inputs and checkboxes for geographic/compliance constraints.
 */
export function SovereigntyFormFields<T extends SovereigntyFormShape>({
  register,
  idPrefix = "apikey",
}: SovereigntyFormFieldsProps<T>) {
  // Cast register to work with our known field names - the parent form
  // is guaranteed to have these fields via the shared schema fragment.
  const reg = register as unknown as UseFormRegister<SovereigntyFormShape>;

  return (
    <div className="col-span-2 border-t pt-4">
      <div className="grid grid-cols-2 gap-x-6 gap-y-4">
        <div className="col-span-2">
          <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-0 flex items-center gap-1">
            Sovereignty Requirements
            <Tooltip>
              <TooltipTrigger asChild>
                <Info className="h-3.5 w-3.5 text-muted-foreground cursor-help" />
              </TooltipTrigger>
              <TooltipContent className="max-w-xs">
                <p>
                  Restrict this key to models matching geographic and compliance constraints. Only
                  models whose provider metadata satisfies all requirements can be used.
                </p>
              </TooltipContent>
            </Tooltip>
          </p>
        </div>
        <FormField
          label="Allowed Inference Countries"
          htmlFor={`${idPrefix}-sov-inference`}
          helpText="Comma-separated ISO country codes"
        >
          <Input
            id={`${idPrefix}-sov-inference`}
            {...reg("sov_inference_countries")}
            placeholder="US, DE, FR"
          />
        </FormField>
        <FormField
          label="Blocked HQ Countries"
          htmlFor={`${idPrefix}-sov-blocked`}
          helpText="Reject providers headquartered in these countries"
        >
          <Input
            id={`${idPrefix}-sov-blocked`}
            {...reg("sov_blocked_countries")}
            placeholder="CN, RU"
          />
        </FormField>
        <FormField
          label="Required Certifications"
          htmlFor={`${idPrefix}-sov-certs`}
          helpText="Provider must have ALL listed certifications"
        >
          <Input
            id={`${idPrefix}-sov-certs`}
            {...reg("sov_certifications")}
            placeholder="gdpr, soc2, hipaa-baa"
          />
        </FormField>
        <FormField
          label="Allowed Licenses"
          htmlFor={`${idPrefix}-sov-licenses`}
          helpText="Only allow models with these licenses"
        >
          <Input
            id={`${idPrefix}-sov-licenses`}
            {...reg("sov_licenses")}
            placeholder="apache-2.0, mit, proprietary"
          />
        </FormField>
        <div className="col-span-2">
          <div className="flex gap-6">
            <label className="flex items-center gap-2 text-sm cursor-pointer">
              <input
                type="checkbox"
                {...reg("sov_require_on_prem")}
                className="rounded border-input"
              />
              Require On-Prem
            </label>
            <label className="flex items-center gap-2 text-sm cursor-pointer">
              <input
                type="checkbox"
                {...reg("sov_require_open_weights")}
                className="rounded border-input"
              />
              Require Open Weights
            </label>
          </div>
        </div>
      </div>
    </div>
  );
}
