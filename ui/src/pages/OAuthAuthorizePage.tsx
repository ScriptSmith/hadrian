import { useEffect, useMemo, useState } from "react";
import { useLocation } from "react-router-dom";
import { Controller, useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { useQuery } from "@tanstack/react-query";
import { ShieldCheck, ExternalLink, Check, X } from "lucide-react";

import { oauthAuthorize } from "@/api/generated/sdk.gen";
import {
  meEligibleOwnersOptions,
  oauthPreflightOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type { ApiKeyOwner, PkceCodeChallengeMethod } from "@/api/generated/types.gen";
import { useAuth } from "@/auth";
import { FormField } from "@/components/FormField/FormField";
import { Select } from "@/components/Select/Select";
import {
  API_KEY_SCOPES,
  ApiKeyOptionsFields,
  type ApiKeyOptionsFormValues,
  buildApiKeyOptionsPayload,
  validateCidrNotation,
  validateModelPatterns,
} from "@/components/Admin/ApiKeyFormModal/apiKeyOptionsFields";
import {
  sovereigntyDefaults,
  sovereigntySchema,
} from "@/components/Admin/ApiKeyFormModal/sovereigntyFields";
import { Button } from "@/components/Button/Button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/Card/Card";
import { Spinner } from "@/components/Spinner/Spinner";

interface OAuthParams {
  callbackUrl: string;
  codeChallenge: string;
  codeChallengeMethod: PkceCodeChallengeMethod;
  appName: string | null;
  scopes: string[] | null;
  keyName: string | null;
}

interface ParseResult {
  params: OAuthParams | null;
  error: string | null;
}

/** Pull and validate the PKCE params from the URL. */
function parseParams(search: string): ParseResult {
  const q = new URLSearchParams(search);
  const callbackUrl = q.get("callback_url");
  const codeChallenge = q.get("code_challenge");
  const methodRaw = q.get("code_challenge_method") ?? "S256";

  if (!callbackUrl) {
    return { params: null, error: "Missing required parameter: callback_url" };
  }
  if (!codeChallenge) {
    return { params: null, error: "Missing required parameter: code_challenge" };
  }
  if (methodRaw !== "S256" && methodRaw !== "plain") {
    return {
      params: null,
      error: `Unsupported code_challenge_method: ${methodRaw}`,
    };
  }

  let parsedCallback: URL;
  try {
    parsedCallback = new URL(callbackUrl);
  } catch {
    return { params: null, error: "callback_url must be a valid URL" };
  }
  const isLoopback =
    parsedCallback.hostname === "localhost" ||
    parsedCallback.hostname === "127.0.0.1" ||
    parsedCallback.hostname === "[::1]";
  if (
    parsedCallback.protocol !== "https:" &&
    !(parsedCallback.protocol === "http:" && isLoopback)
  ) {
    return {
      params: null,
      error: "callback_url must use https (http is allowed only for loopback hosts)",
    };
  }

  const scopesRaw = q.get("scopes");
  const scopes = scopesRaw
    ? scopesRaw
        .split(",")
        .map((s) => s.trim())
        .filter((s) => s.length > 0)
    : null;

  return {
    params: {
      callbackUrl,
      codeChallenge,
      codeChallengeMethod: methodRaw,
      appName: q.get("app_name"),
      scopes,
      keyName: q.get("key_name"),
    },
    error: null,
  };
}

/**
 * Decode the encoded owner choice ("user" or `${type}:${id}`) into the
 * `ApiKeyOwner` shape the backend expects. `userId` is the consenting
 * user's ID — used when the choice is the default ("user") so we can
 * pin the owner to them explicitly.
 */
function ownerKeyToApiKeyOwner(key: string, userId: string): ApiKeyOwner {
  if (key === "user") {
    return { type: "user", user_id: userId };
  }
  const [kind, id] = key.split(":", 2);
  switch (kind) {
    case "organization":
      return { type: "organization", org_id: id };
    case "team":
      return { type: "team", team_id: id };
    case "project":
      return { type: "project", project_id: id };
    default:
      throw new Error(`Unsupported owner kind: ${kind}`);
  }
}

/** Append `?error=...` (or `&error=...`) to the callback URL. */
function buildErrorRedirect(callbackUrl: string, error: string): string {
  try {
    const url = new URL(callbackUrl);
    url.searchParams.set("error", error);
    return url.toString();
  } catch {
    return callbackUrl;
  }
}

const schema = z
  .object({
    name: z.string().min(1, "Label is required"),
    budget_limit_cents: z.string().optional(),
    budget_period: z.enum(["daily", "monthly", ""]).optional(),
    expires_at: z.string().optional(),
    scopes: z.array(z.string()).optional(),
    allowed_models: z.string().optional(),
    ip_allowlist: z.string().optional(),
    rate_limit_rpm: z.string().optional(),
    rate_limit_tpm: z.string().optional(),
    ...sovereigntySchema,
  })
  .refine((data) => validateModelPatterns(data.allowed_models), {
    message:
      "Invalid model pattern. Use alphanumeric characters, hyphens, dots, slashes, and optional trailing wildcard (*)",
    path: ["allowed_models"],
  })
  .refine((data) => validateCidrNotation(data.ip_allowlist), {
    message: "Invalid IP/CIDR notation. Use format like 192.168.1.1, 10.0.0.0/8, or 2001:db8::/32",
    path: ["ip_allowlist"],
  });

export default function OAuthAuthorizePage() {
  const { isAuthenticated, isLoading: authLoading, user } = useAuth();
  const location = useLocation();

  const { params, error: parseError } = useMemo(
    () => parseParams(location.search),
    [location.search]
  );
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  /** Encoded owner choice ("user" or `${type}:${id}`). */
  const [ownerKey, setOwnerKey] = useState<string>("user");
  /**
   * Two-page wizard. Page 1 reviews/edits the scopes the app will get;
   * page 2 collects the rest of the API key options (label, owner, budget,
   * expiry, etc.) and actually submits.
   *
   * Both pages share the same react-hook-form instance, so scopes picked
   * on page 1 carry through to the Permission Scopes field that lives in
   * `ApiKeyOptionsFields` on page 2.
   */
  const [step, setStep] = useState<1 | 2>(1);

  // Fetch the orgs/teams/projects the user could pick as the key owner.
  // Disabled until the user is authenticated to avoid 401 noise.
  const eligibleOwnersQuery = useQuery({
    ...meEligibleOwnersOptions(),
    enabled: isAuthenticated && !authLoading,
  });

  // Server-side preflight: ask the backend whether `callback_url` passes
  // the deployment's allow/deny lists *before* we render the consent UI.
  // Without this, a user clicking "Deny" would still get redirected to a
  // host that the operator denied, since handleDeny is purely client-side.
  const preflightQuery = useQuery({
    ...oauthPreflightOptions({
      query: { callback_url: params?.callbackUrl ?? "" },
    }),
    enabled: isAuthenticated && !authLoading && !!params?.callbackUrl,
    retry: false,
  });

  // Default the label to whatever the app suggested, falling back to its
  // display name. The user can edit it before clicking Authorize.
  const defaultName = useMemo(() => {
    return params?.keyName?.trim() || params?.appName?.trim() || "";
  }, [params?.keyName, params?.appName]);

  const form = useForm<ApiKeyOptionsFormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      name: defaultName,
      budget_limit_cents: "",
      budget_period: "",
      expires_at: "",
      scopes: params?.scopes ?? [],
      allowed_models: "",
      ip_allowlist: "",
      rate_limit_rpm: "",
      rate_limit_tpm: "",
      ...sovereigntyDefaults,
    },
  });

  // Reset the form whenever the URL params change so app-suggested defaults
  // (label, scopes) take effect on first render and after navigation.
  useEffect(() => {
    form.reset({
      name: defaultName,
      budget_limit_cents: "",
      budget_period: "",
      expires_at: "",
      scopes: params?.scopes ?? [],
      allowed_models: "",
      ip_allowlist: "",
      rate_limit_rpm: "",
      rate_limit_tpm: "",
      ...sovereigntyDefaults,
    });
  }, [defaultName, params?.scopes, form]);

  const selectedScopes = form.watch("scopes") || [];

  // If unauthenticated, send the user to /login but preserve the FULL URL
  // (including the query string) so they return here after sign-in.
  useEffect(() => {
    if (authLoading || isAuthenticated) {
      return;
    }
    const returnTo = `${location.pathname}${location.search}`;
    window.location.href = `/login?return_to=${encodeURIComponent(returnTo)}`;
  }, [authLoading, isAuthenticated, location.pathname, location.search]);

  if (authLoading || !isAuthenticated) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-background">
        <Spinner size="lg" />
      </div>
    );
  }

  if (parseError || !params) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-background p-4">
        <Card className="w-full max-w-md">
          <CardHeader>
            <CardTitle>Invalid authorization request</CardTitle>
            <CardDescription>The link you followed is missing required parameters.</CardDescription>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-destructive">{parseError ?? "Unknown error"}</p>
          </CardContent>
        </Card>
      </div>
    );
  }

  // Wait for the server-side preflight before rendering the consent UI.
  // This is what makes the deny button safe — if the URL is denied, we
  // never give the user a button to redirect to it.
  if (preflightQuery.isLoading) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-background">
        <Spinner size="lg" />
      </div>
    );
  }
  if (preflightQuery.isError) {
    const err = preflightQuery.error as { message?: unknown } | undefined;
    const message =
      (typeof err?.message === "string" && err.message) ||
      "This callback URL is not permitted by the server's OAuth policy.";
    return (
      <div className="flex min-h-screen items-center justify-center bg-background p-4">
        <Card className="w-full max-w-md">
          <CardHeader>
            <CardTitle>Authorization request rejected</CardTitle>
            <CardDescription>The callback URL is not allowed.</CardDescription>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-destructive">{message}</p>
          </CardContent>
        </Card>
      </div>
    );
  }

  const callbackHost = (() => {
    try {
      return new URL(params.callbackUrl).host;
    } catch {
      return params.callbackUrl;
    }
  })();
  const appLabel = params.appName?.trim() || callbackHost;

  const handleAllow = form.handleSubmit(async (data) => {
    setSubmitting(true);
    setSubmitError(null);
    try {
      const payload = buildApiKeyOptionsPayload(data);
      const owner = user?.id ? ownerKeyToApiKeyOwner(ownerKey, user.id) : undefined;
      const response = await oauthAuthorize({
        body: {
          callback_url: params.callbackUrl,
          code_challenge: params.codeChallenge,
          code_challenge_method: params.codeChallengeMethod,
          app_name: params.appName ?? undefined,
          key_options: {
            owner,
            name: payload.name,
            budget_limit_cents: payload.budget_limit_cents,
            budget_period: payload.budget_period,
            expires_at: payload.expires_at,
            scopes: payload.scopes,
            allowed_models: payload.allowed_models,
            ip_allowlist: payload.ip_allowlist,
            rate_limit_rpm: payload.rate_limit_rpm,
            rate_limit_tpm: payload.rate_limit_tpm,
            ...(payload.sovereignty_requirements && {
              sovereignty_requirements: payload.sovereignty_requirements,
            }),
          },
        },
      });
      if (response.error) {
        const message =
          (typeof response.error === "object" &&
            response.error !== null &&
            "message" in response.error &&
            typeof (response.error as { message?: unknown }).message === "string" &&
            (response.error as { message: string }).message) ||
          "Failed to authorize app";
        throw new Error(message);
      }
      if (!response.data?.redirect_url) {
        throw new Error("Authorization succeeded but no redirect URL was returned");
      }
      window.location.href = response.data.redirect_url;
    } catch (err) {
      setSubmitError(err instanceof Error ? err.message : "Failed to authorize app");
      setSubmitting(false);
    }
  });

  const handleDeny = () => {
    window.location.href = buildErrorRedirect(params.callbackUrl, "access_denied");
  };

  return (
    <div className="flex min-h-screen items-center justify-center bg-background p-4">
      <Card className="w-full max-w-2xl">
        <CardHeader className="text-center">
          <div className="mx-auto mb-3 flex h-12 w-12 items-center justify-center rounded-full bg-primary/10">
            <ShieldCheck className="h-6 w-6 text-primary" aria-hidden="true" />
          </div>
          <CardTitle className="text-xl">Authorize {appLabel}</CardTitle>
          <CardDescription>
            This app is requesting an API key tied to your account, which will give it access to the
            Hadrian resources you grant. Make sure you recognize the app's URL:
          </CardDescription>
          <div className="mx-auto mt-3 flex w-full max-w-md items-center justify-center gap-2 rounded-md border bg-muted/40 px-3 py-2 text-sm">
            <ExternalLink className="h-4 w-4 text-muted-foreground" aria-hidden="true" />
            <span className="break-all font-mono text-xs">{params.callbackUrl}</span>
          </div>
          {user?.email && (
            <p className="mt-3 text-sm text-muted-foreground">
              Signed in as <span className="font-medium text-foreground">{user.email}</span>
            </p>
          )}
        </CardHeader>
        <CardContent className="space-y-5">
          {/* Step indicator */}
          <ol className="flex items-center justify-center gap-3 text-xs text-muted-foreground">
            <li
              className={step === 1 ? "font-semibold text-foreground" : undefined}
              aria-current={step === 1 ? "step" : undefined}
            >
              1. Scopes
            </li>
            <li aria-hidden="true">→</li>
            <li
              className={step === 2 ? "font-semibold text-foreground" : undefined}
              aria-current={step === 2 ? "step" : undefined}
            >
              2. Key details
            </li>
          </ol>

          {step === 1 ? (
            <>
              <p className="text-sm text-muted-foreground">
                Pick which Hadrian APIs{" "}
                <span className="font-medium text-foreground">{appLabel}</span> can use with the
                issued key. Leaving Permission Scopes empty grants full access.
              </p>

              <FormField
                label="Permission Scopes"
                htmlFor="oauth-consent-step1-scopes"
                helpText={
                  selectedScopes.length > 0
                    ? `${selectedScopes.length} scope${selectedScopes.length === 1 ? "" : "s"} selected`
                    : "No restrictions (full access)"
                }
              >
                <Controller
                  name="scopes"
                  control={form.control}
                  render={({ field }) => (
                    <Select
                      multiple
                      options={API_KEY_SCOPES}
                      value={field.value || []}
                      onChange={field.onChange}
                      placeholder="Select scopes..."
                      searchable
                    />
                  )}
                />
              </FormField>

              {(() => {
                // No selection means the issued key has full access — every
                // scope is effectively granted.
                const grantsAll = selectedScopes.length === 0;
                return (
                  <div className="rounded-md border bg-muted/30 px-3 py-3 text-sm">
                    <p className="text-xs font-medium">
                      {grantsAll ? "Scopes (full access)" : "Scopes"}
                    </p>
                    <ul className="mt-2 grid grid-cols-1 gap-x-4 gap-y-1 text-xs sm:grid-cols-2">
                      {API_KEY_SCOPES.map((s) => {
                        const granted = grantsAll || selectedScopes.includes(s.value);
                        return (
                          <li key={s.value} className="flex items-start gap-2">
                            {granted ? (
                              <Check
                                className="mt-0.5 h-3.5 w-3.5 shrink-0 text-emerald-600 dark:text-emerald-400"
                                aria-label="granted"
                              />
                            ) : (
                              <X
                                className="mt-0.5 h-3.5 w-3.5 shrink-0 text-destructive"
                                aria-label="not granted"
                              />
                            )}
                            <span
                              className={
                                granted
                                  ? "font-mono font-medium text-foreground"
                                  : "font-mono font-medium text-foreground line-through"
                              }
                            >
                              {s.value}
                            </span>
                            <span className="text-muted-foreground">{s.description}</span>
                          </li>
                        );
                      })}
                    </ul>
                  </div>
                );
              })()}

              <div className="flex gap-3 pt-2">
                <Button type="button" variant="outline" className="flex-1" onClick={handleDeny}>
                  Deny
                </Button>
                <Button type="button" className="flex-1" onClick={() => setStep(2)}>
                  Continue
                </Button>
              </div>
            </>
          ) : (
            <form onSubmit={handleAllow} className="space-y-5">
              <p className="text-sm text-muted-foreground">
                Confirm who owns the issued key and tweak its label, budget, and expiry.
              </p>

              <FormField
                label="Owner"
                htmlFor="oauth-consent-owner"
                helpText={
                  eligibleOwnersQuery.isError
                    ? "Could not load other owners — issuing under your personal account."
                    : "The org, team, or project the issued key will belong to."
                }
              >
                <select
                  id="oauth-consent-owner"
                  className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                  value={ownerKey}
                  onChange={(e) => setOwnerKey(e.target.value)}
                  disabled={eligibleOwnersQuery.isLoading}
                >
                  <option value="user">Personal {user?.email ? `— ${user.email}` : ""}</option>
                  {eligibleOwnersQuery.data?.organizations &&
                    eligibleOwnersQuery.data.organizations.length > 0 && (
                      <optgroup label="Organizations">
                        {eligibleOwnersQuery.data.organizations.map((o) => (
                          <option key={`organization:${o.id}`} value={`organization:${o.id}`}>
                            {o.name}
                          </option>
                        ))}
                      </optgroup>
                    )}
                  {eligibleOwnersQuery.data?.teams && eligibleOwnersQuery.data.teams.length > 0 && (
                    <optgroup label="Teams">
                      {eligibleOwnersQuery.data.teams.map((t) => (
                        <option key={`team:${t.id}`} value={`team:${t.id}`}>
                          {t.org_slug ? `${t.org_slug} / ${t.name}` : t.name}
                        </option>
                      ))}
                    </optgroup>
                  )}
                  {eligibleOwnersQuery.data?.projects &&
                    eligibleOwnersQuery.data.projects.length > 0 && (
                      <optgroup label="Projects">
                        {eligibleOwnersQuery.data.projects.map((p) => (
                          <option key={`project:${p.id}`} value={`project:${p.id}`}>
                            {p.org_slug ? `${p.org_slug} / ${p.name}` : p.name}
                          </option>
                        ))}
                      </optgroup>
                    )}
                </select>
              </FormField>

              <ApiKeyOptionsFields
                register={form.register}
                control={form.control}
                errors={form.formState.errors}
                selectedScopes={selectedScopes}
                idPrefix="oauth-consent"
                namePlaceholder={appLabel}
                // Page 1 already collected scopes; the picker still lives in
                // Advanced Settings here so the user can tweak it, but it
                // starts collapsed since they just confirmed it.
                advancedDefaultOpen={false}
              />

              <p className="text-xs text-muted-foreground">
                The issued key is owned by the selected owner and can be revoked at any time from
                the API Keys page.
              </p>

              {submitError && (
                <div
                  role="alert"
                  className="rounded-md bg-destructive/10 p-3 text-sm text-destructive"
                >
                  {submitError}
                </div>
              )}

              <div className="flex gap-3 pt-2">
                <Button
                  type="button"
                  variant="outline"
                  className="flex-1"
                  onClick={() => setStep(1)}
                  disabled={submitting}
                >
                  Back
                </Button>
                <Button type="submit" className="flex-1" disabled={submitting}>
                  {submitting ? "Authorizing..." : "Authorize"}
                </Button>
              </div>
            </form>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
