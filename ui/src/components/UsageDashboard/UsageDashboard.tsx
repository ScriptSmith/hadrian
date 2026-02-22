import { useQuery } from "@tanstack/react-query";
import {
  Building2,
  Calendar,
  Cpu,
  DollarSign,
  FolderKanban,
  ImageIcon,
  Layers,
  Mic,
  PieChartIcon,
  Server,
  Tag,
  TrendingUp,
  User,
  Users,
} from "lucide-react";
import { useState, useMemo, useCallback, useEffect } from "react";

import {
  usageGetSummaryOptions,
  usageGetByDateOptions,
  usageGetByModelOptions,
  usageGetByProviderOptions,
  usageGetByDateModelOptions,
  usageGetByDateProviderOptions,
  usageGetByPricingSourceOptions,
  usageGetByDatePricingSourceOptions,
  usageGetOrgSummaryOptions,
  usageGetOrgByDateOptions,
  usageGetOrgByModelOptions,
  usageGetOrgByProviderOptions,
  usageGetOrgByDateModelOptions,
  usageGetOrgByDateProviderOptions,
  usageGetOrgByPricingSourceOptions,
  usageGetOrgByDatePricingSourceOptions,
  usageGetOrgByUserOptions,
  usageGetOrgByDateUserOptions,
  usageGetOrgByProjectOptions,
  usageGetOrgByDateProjectOptions,
  usageGetOrgByTeamOptions,
  usageGetOrgByDateTeamOptions,
  usageGetProjectSummaryOptions,
  usageGetProjectByDateOptions,
  usageGetProjectByModelOptions,
  usageGetProjectByProviderOptions,
  usageGetProjectByDateModelOptions,
  usageGetProjectByDateProviderOptions,
  usageGetProjectByPricingSourceOptions,
  usageGetProjectByDatePricingSourceOptions,
  usageGetProjectByUserOptions,
  usageGetProjectByDateUserOptions,
  usageGetTeamSummaryOptions,
  usageGetTeamByDateOptions,
  usageGetTeamByModelOptions,
  usageGetTeamByProviderOptions,
  usageGetTeamByDateModelOptions,
  usageGetTeamByDateProviderOptions,
  usageGetTeamByPricingSourceOptions,
  usageGetTeamByDatePricingSourceOptions,
  usageGetTeamByUserOptions,
  usageGetTeamByDateUserOptions,
  usageGetTeamByProjectOptions,
  usageGetTeamByDateProjectOptions,
  usageGetUserSummaryOptions,
  usageGetUserByDateOptions,
  usageGetUserByModelOptions,
  usageGetUserByProviderOptions,
  usageGetUserByDateModelOptions,
  usageGetUserByDateProviderOptions,
  usageGetUserByPricingSourceOptions,
  usageGetUserByDatePricingSourceOptions,
  usageGetGlobalSummaryOptions,
  usageGetGlobalByDateOptions,
  usageGetGlobalByModelOptions,
  usageGetGlobalByProviderOptions,
  usageGetGlobalByPricingSourceOptions,
  usageGetGlobalByDateModelOptions,
  usageGetGlobalByDateProviderOptions,
  usageGetGlobalByDatePricingSourceOptions,
  usageGetGlobalByUserOptions,
  usageGetGlobalByDateUserOptions,
  usageGetGlobalByProjectOptions,
  usageGetGlobalByDateProjectOptions,
  usageGetGlobalByTeamOptions,
  usageGetGlobalByDateTeamOptions,
  usageGetGlobalByOrgOptions,
  usageGetGlobalByDateOrgOptions,
  meUsageSummaryOptions,
  meUsageByDateOptions,
  meUsageByModelOptions,
  meUsageByProviderOptions,
  meUsageByDateModelOptions,
  meUsageByDateProviderOptions,
  meUsageByPricingSourceOptions,
  meUsageByDatePricingSourceOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type {
  UsageSummaryResponse,
  DailySpendResponse,
  ModelSpendResponse,
  ProviderSpendResponse,
  DailyModelSpendResponse,
  DailyProviderSpendResponse,
  PricingSourceSpendResponse,
  DailyPricingSourceSpendResponse,
  UserSpendResponse,
  DailyUserSpendResponse,
  ProjectSpendResponse,
  DailyProjectSpendResponse,
  TeamSpendResponse,
  DailyTeamSpendResponse,
  OrgSpendResponse,
  DailyOrgSpendResponse,
} from "@/api/generated/types.gen";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import {
  LineChart,
  StackedBarChart,
  PieChart,
  ChartLegend,
  Sparkline,
  CHART_COLORS,
  type ChartSeries,
} from "@/components/Charts";
import {
  StatCard,
  StatValue,
  DateRangeFilter,
  getDefaultDateRange,
  type DateRange,
} from "@/components/Admin";
import { Badge } from "@/components/Badge/Badge";
import { formatCurrency, formatNumber, formatDateTime } from "@/utils/formatters";

export type UsageScope =
  | { type: "apiKey"; keyId: string }
  | { type: "organization"; slug: string }
  | { type: "project"; orgSlug: string; projectSlug: string }
  | { type: "team"; orgSlug: string; teamSlug: string }
  | { type: "user"; userId: string }
  | { type: "me" }
  | { type: "global" };

export type ChartMetric =
  | "cost"
  | "tokens"
  | "requests"
  | "input_tokens"
  | "output_tokens"
  | "images"
  | "audio";
export type GroupBy =
  | "none"
  | "model"
  | "provider"
  | "pricing_source"
  | "user"
  | "project"
  | "team"
  | "organization";

interface UsageDashboardProps {
  scope: UsageScope;
}

const METRIC_OPTIONS: { value: ChartMetric; label: string }[] = [
  { value: "cost", label: "Cost" },
  { value: "tokens", label: "Tokens" },
  { value: "requests", label: "Requests" },
  { value: "input_tokens", label: "Input" },
  { value: "output_tokens", label: "Output" },
  { value: "images", label: "Images" },
  { value: "audio", label: "Audio" },
];

const BASE_GROUP_BY_OPTIONS: { value: GroupBy; label: string }[] = [
  { value: "none", label: "None" },
  { value: "model", label: "Model" },
  { value: "provider", label: "Provider" },
  { value: "pricing_source", label: "Pricing Source" },
];

function getGroupByOptions(scope: UsageScope): { value: GroupBy; label: string }[] {
  switch (scope.type) {
    case "project":
      return [...BASE_GROUP_BY_OPTIONS, { value: "user", label: "User" }];
    case "team":
      return [
        ...BASE_GROUP_BY_OPTIONS,
        { value: "user", label: "User" },
        { value: "project", label: "Project" },
      ];
    case "organization":
      return [
        ...BASE_GROUP_BY_OPTIONS,
        { value: "user", label: "User" },
        { value: "project", label: "Project" },
        { value: "team", label: "Team" },
      ];
    case "global":
      return [
        ...BASE_GROUP_BY_OPTIONS,
        { value: "user", label: "User" },
        { value: "project", label: "Project" },
        { value: "team", label: "Team" },
        { value: "organization", label: "Organization" },
      ];
    default:
      return BASE_GROUP_BY_OPTIONS;
  }
}

function ToggleButtons<T extends string>({
  options,
  value,
  onChange,
  label,
}: {
  options: { value: T; label: string }[];
  value: T;
  onChange: (v: T) => void;
  label: string;
}) {
  return (
    <div className="flex items-center gap-2" role="radiogroup" aria-label={label}>
      {options.map((opt) => (
        <button
          key={opt.value}
          role="radio"
          aria-checked={value === opt.value}
          onClick={() => onChange(opt.value)}
          className={`rounded-md px-2.5 py-1 text-xs font-medium transition-colors ${
            value === opt.value
              ? "bg-primary text-primary-foreground"
              : "bg-muted text-muted-foreground hover:text-foreground"
          }`}
        >
          {opt.label}
        </button>
      ))}
    </div>
  );
}

/**
 * Hook that dispatches usage queries to the appropriate scope-specific endpoint.
 *
 * Each scope type has its own `useQuery` call gated by `enabled` to avoid
 * TypeScript union issues across different generated option types.
 * React Query deduplicates based on query keys, so disabled queries are no-ops.
 */
function useScopedUsageQueries(scope: UsageScope, dateRange: DateRange, groupBy: GroupBy) {
  const query = { start_date: dateRange.start_date, end_date: dateRange.end_date };
  const t = scope.type;

  // --- Summary ---
  const apiKeySummary = useQuery({
    ...usageGetSummaryOptions({
      path: { key_id: t === "apiKey" ? scope.keyId : "" },
      query,
    }),
    enabled: t === "apiKey",
  });
  const orgSummary = useQuery({
    ...usageGetOrgSummaryOptions({
      path: { slug: t === "organization" ? scope.slug : "" },
      query,
    }),
    enabled: t === "organization",
  });
  const projectSummary = useQuery({
    ...usageGetProjectSummaryOptions({
      path: {
        org_slug: t === "project" ? scope.orgSlug : "",
        project_slug: t === "project" ? scope.projectSlug : "",
      },
      query,
    }),
    enabled: t === "project",
  });
  const teamSummary = useQuery({
    ...usageGetTeamSummaryOptions({
      path: {
        org_slug: t === "team" ? scope.orgSlug : "",
        team_slug: t === "team" ? scope.teamSlug : "",
      },
      query,
    }),
    enabled: t === "team",
  });
  const userSummary = useQuery({
    ...usageGetUserSummaryOptions({
      path: { user_id: t === "user" ? scope.userId : "" },
      query,
    }),
    enabled: t === "user",
  });
  const meSummary = useQuery({
    ...meUsageSummaryOptions({ query }),
    enabled: t === "me",
  });
  const globalSummary = useQuery({
    ...usageGetGlobalSummaryOptions({ query }),
    enabled: t === "global",
  });

  // --- By Date ---
  const apiKeyByDate = useQuery({
    ...usageGetByDateOptions({
      path: { key_id: t === "apiKey" ? scope.keyId : "" },
      query,
    }),
    enabled: t === "apiKey",
  });
  const orgByDate = useQuery({
    ...usageGetOrgByDateOptions({
      path: { slug: t === "organization" ? scope.slug : "" },
      query,
    }),
    enabled: t === "organization",
  });
  const projectByDate = useQuery({
    ...usageGetProjectByDateOptions({
      path: {
        org_slug: t === "project" ? scope.orgSlug : "",
        project_slug: t === "project" ? scope.projectSlug : "",
      },
      query,
    }),
    enabled: t === "project",
  });
  const teamByDate = useQuery({
    ...usageGetTeamByDateOptions({
      path: {
        org_slug: t === "team" ? scope.orgSlug : "",
        team_slug: t === "team" ? scope.teamSlug : "",
      },
      query,
    }),
    enabled: t === "team",
  });
  const userByDate = useQuery({
    ...usageGetUserByDateOptions({
      path: { user_id: t === "user" ? scope.userId : "" },
      query,
    }),
    enabled: t === "user",
  });
  const meByDate = useQuery({
    ...meUsageByDateOptions({ query }),
    enabled: t === "me",
  });
  const globalByDate = useQuery({
    ...usageGetGlobalByDateOptions({ query }),
    enabled: t === "global",
  });

  // --- By Model ---
  const apiKeyByModel = useQuery({
    ...usageGetByModelOptions({
      path: { key_id: t === "apiKey" ? scope.keyId : "" },
      query,
    }),
    enabled: t === "apiKey",
  });
  const orgByModel = useQuery({
    ...usageGetOrgByModelOptions({
      path: { slug: t === "organization" ? scope.slug : "" },
      query,
    }),
    enabled: t === "organization",
  });
  const projectByModel = useQuery({
    ...usageGetProjectByModelOptions({
      path: {
        org_slug: t === "project" ? scope.orgSlug : "",
        project_slug: t === "project" ? scope.projectSlug : "",
      },
      query,
    }),
    enabled: t === "project",
  });
  const teamByModel = useQuery({
    ...usageGetTeamByModelOptions({
      path: {
        org_slug: t === "team" ? scope.orgSlug : "",
        team_slug: t === "team" ? scope.teamSlug : "",
      },
      query,
    }),
    enabled: t === "team",
  });
  const userByModel = useQuery({
    ...usageGetUserByModelOptions({
      path: { user_id: t === "user" ? scope.userId : "" },
      query,
    }),
    enabled: t === "user",
  });
  const meByModel = useQuery({
    ...meUsageByModelOptions({ query }),
    enabled: t === "me",
  });
  const globalByModel = useQuery({
    ...usageGetGlobalByModelOptions({ query }),
    enabled: t === "global",
  });

  // --- By Provider ---
  const apiKeyByProvider = useQuery({
    ...usageGetByProviderOptions({
      path: { key_id: t === "apiKey" ? scope.keyId : "" },
      query,
    }),
    enabled: t === "apiKey",
  });
  const orgByProvider = useQuery({
    ...usageGetOrgByProviderOptions({
      path: { slug: t === "organization" ? scope.slug : "" },
      query,
    }),
    enabled: t === "organization",
  });
  const projectByProvider = useQuery({
    ...usageGetProjectByProviderOptions({
      path: {
        org_slug: t === "project" ? scope.orgSlug : "",
        project_slug: t === "project" ? scope.projectSlug : "",
      },
      query,
    }),
    enabled: t === "project",
  });
  const teamByProvider = useQuery({
    ...usageGetTeamByProviderOptions({
      path: {
        org_slug: t === "team" ? scope.orgSlug : "",
        team_slug: t === "team" ? scope.teamSlug : "",
      },
      query,
    }),
    enabled: t === "team",
  });
  const userByProvider = useQuery({
    ...usageGetUserByProviderOptions({
      path: { user_id: t === "user" ? scope.userId : "" },
      query,
    }),
    enabled: t === "user",
  });
  const meByProvider = useQuery({
    ...meUsageByProviderOptions({ query }),
    enabled: t === "me",
  });
  const globalByProvider = useQuery({
    ...usageGetGlobalByProviderOptions({ query }),
    enabled: t === "global",
  });

  // --- By Date+Model (only fetched when groupBy is "model") ---
  const apiKeyByDateModel = useQuery({
    ...usageGetByDateModelOptions({
      path: { key_id: t === "apiKey" ? scope.keyId : "" },
      query,
    }),
    enabled: t === "apiKey" && groupBy === "model",
  });
  const orgByDateModel = useQuery({
    ...usageGetOrgByDateModelOptions({
      path: { slug: t === "organization" ? scope.slug : "" },
      query,
    }),
    enabled: t === "organization" && groupBy === "model",
  });
  const projectByDateModel = useQuery({
    ...usageGetProjectByDateModelOptions({
      path: {
        org_slug: t === "project" ? scope.orgSlug : "",
        project_slug: t === "project" ? scope.projectSlug : "",
      },
      query,
    }),
    enabled: t === "project" && groupBy === "model",
  });
  const teamByDateModel = useQuery({
    ...usageGetTeamByDateModelOptions({
      path: {
        org_slug: t === "team" ? scope.orgSlug : "",
        team_slug: t === "team" ? scope.teamSlug : "",
      },
      query,
    }),
    enabled: t === "team" && groupBy === "model",
  });
  const userByDateModel = useQuery({
    ...usageGetUserByDateModelOptions({
      path: { user_id: t === "user" ? scope.userId : "" },
      query,
    }),
    enabled: t === "user" && groupBy === "model",
  });
  const meByDateModel = useQuery({
    ...meUsageByDateModelOptions({ query }),
    enabled: t === "me" && groupBy === "model",
  });
  const globalByDateModel = useQuery({
    ...usageGetGlobalByDateModelOptions({ query }),
    enabled: t === "global" && groupBy === "model",
  });

  // --- By Date+Provider (only fetched when groupBy is "provider") ---
  const apiKeyByDateProvider = useQuery({
    ...usageGetByDateProviderOptions({
      path: { key_id: t === "apiKey" ? scope.keyId : "" },
      query,
    }),
    enabled: t === "apiKey" && groupBy === "provider",
  });
  const orgByDateProvider = useQuery({
    ...usageGetOrgByDateProviderOptions({
      path: { slug: t === "organization" ? scope.slug : "" },
      query,
    }),
    enabled: t === "organization" && groupBy === "provider",
  });
  const projectByDateProvider = useQuery({
    ...usageGetProjectByDateProviderOptions({
      path: {
        org_slug: t === "project" ? scope.orgSlug : "",
        project_slug: t === "project" ? scope.projectSlug : "",
      },
      query,
    }),
    enabled: t === "project" && groupBy === "provider",
  });
  const teamByDateProvider = useQuery({
    ...usageGetTeamByDateProviderOptions({
      path: {
        org_slug: t === "team" ? scope.orgSlug : "",
        team_slug: t === "team" ? scope.teamSlug : "",
      },
      query,
    }),
    enabled: t === "team" && groupBy === "provider",
  });
  const userByDateProvider = useQuery({
    ...usageGetUserByDateProviderOptions({
      path: { user_id: t === "user" ? scope.userId : "" },
      query,
    }),
    enabled: t === "user" && groupBy === "provider",
  });
  const meByDateProvider = useQuery({
    ...meUsageByDateProviderOptions({ query }),
    enabled: t === "me" && groupBy === "provider",
  });
  const globalByDateProvider = useQuery({
    ...usageGetGlobalByDateProviderOptions({ query }),
    enabled: t === "global" && groupBy === "provider",
  });

  // --- By Pricing Source ---
  const apiKeyByPricingSource = useQuery({
    ...usageGetByPricingSourceOptions({
      path: { key_id: t === "apiKey" ? scope.keyId : "" },
      query,
    }),
    enabled: t === "apiKey",
  });
  const orgByPricingSource = useQuery({
    ...usageGetOrgByPricingSourceOptions({
      path: { slug: t === "organization" ? scope.slug : "" },
      query,
    }),
    enabled: t === "organization",
  });
  const projectByPricingSource = useQuery({
    ...usageGetProjectByPricingSourceOptions({
      path: {
        org_slug: t === "project" ? scope.orgSlug : "",
        project_slug: t === "project" ? scope.projectSlug : "",
      },
      query,
    }),
    enabled: t === "project",
  });
  const teamByPricingSource = useQuery({
    ...usageGetTeamByPricingSourceOptions({
      path: {
        org_slug: t === "team" ? scope.orgSlug : "",
        team_slug: t === "team" ? scope.teamSlug : "",
      },
      query,
    }),
    enabled: t === "team",
  });
  const userByPricingSource = useQuery({
    ...usageGetUserByPricingSourceOptions({
      path: { user_id: t === "user" ? scope.userId : "" },
      query,
    }),
    enabled: t === "user",
  });
  const meByPricingSource = useQuery({
    ...meUsageByPricingSourceOptions({ query }),
    enabled: t === "me",
  });
  const globalByPricingSource = useQuery({
    ...usageGetGlobalByPricingSourceOptions({ query }),
    enabled: t === "global",
  });

  // --- By Date+Pricing Source (only fetched when groupBy is "pricing_source") ---
  const apiKeyByDatePricingSource = useQuery({
    ...usageGetByDatePricingSourceOptions({
      path: { key_id: t === "apiKey" ? scope.keyId : "" },
      query,
    }),
    enabled: t === "apiKey" && groupBy === "pricing_source",
  });
  const orgByDatePricingSource = useQuery({
    ...usageGetOrgByDatePricingSourceOptions({
      path: { slug: t === "organization" ? scope.slug : "" },
      query,
    }),
    enabled: t === "organization" && groupBy === "pricing_source",
  });
  const projectByDatePricingSource = useQuery({
    ...usageGetProjectByDatePricingSourceOptions({
      path: {
        org_slug: t === "project" ? scope.orgSlug : "",
        project_slug: t === "project" ? scope.projectSlug : "",
      },
      query,
    }),
    enabled: t === "project" && groupBy === "pricing_source",
  });
  const teamByDatePricingSource = useQuery({
    ...usageGetTeamByDatePricingSourceOptions({
      path: {
        org_slug: t === "team" ? scope.orgSlug : "",
        team_slug: t === "team" ? scope.teamSlug : "",
      },
      query,
    }),
    enabled: t === "team" && groupBy === "pricing_source",
  });
  const userByDatePricingSource = useQuery({
    ...usageGetUserByDatePricingSourceOptions({
      path: { user_id: t === "user" ? scope.userId : "" },
      query,
    }),
    enabled: t === "user" && groupBy === "pricing_source",
  });
  const meByDatePricingSource = useQuery({
    ...meUsageByDatePricingSourceOptions({ query }),
    enabled: t === "me" && groupBy === "pricing_source",
  });
  const globalByDatePricingSource = useQuery({
    ...usageGetGlobalByDatePricingSourceOptions({ query }),
    enabled: t === "global" && groupBy === "pricing_source",
  });

  // --- Entity breakdowns: By User ---
  const supportsUser = t === "project" || t === "team" || t === "organization" || t === "global";
  const projectByUser = useQuery({
    ...usageGetProjectByUserOptions({
      path: {
        org_slug: t === "project" ? scope.orgSlug : "",
        project_slug: t === "project" ? scope.projectSlug : "",
      },
      query,
    }),
    enabled: t === "project" && supportsUser,
  });
  const teamByUser = useQuery({
    ...usageGetTeamByUserOptions({
      path: {
        org_slug: t === "team" ? scope.orgSlug : "",
        team_slug: t === "team" ? scope.teamSlug : "",
      },
      query,
    }),
    enabled: t === "team" && supportsUser,
  });
  const orgByUser = useQuery({
    ...usageGetOrgByUserOptions({
      path: { slug: t === "organization" ? scope.slug : "" },
      query,
    }),
    enabled: t === "organization" && supportsUser,
  });
  const globalByUser = useQuery({
    ...usageGetGlobalByUserOptions({ query }),
    enabled: t === "global" && supportsUser,
  });

  // --- Entity breakdowns: By Date+User ---
  const projectByDateUser = useQuery({
    ...usageGetProjectByDateUserOptions({
      path: {
        org_slug: t === "project" ? scope.orgSlug : "",
        project_slug: t === "project" ? scope.projectSlug : "",
      },
      query,
    }),
    enabled: t === "project" && groupBy === "user",
  });
  const teamByDateUser = useQuery({
    ...usageGetTeamByDateUserOptions({
      path: {
        org_slug: t === "team" ? scope.orgSlug : "",
        team_slug: t === "team" ? scope.teamSlug : "",
      },
      query,
    }),
    enabled: t === "team" && groupBy === "user",
  });
  const orgByDateUser = useQuery({
    ...usageGetOrgByDateUserOptions({
      path: { slug: t === "organization" ? scope.slug : "" },
      query,
    }),
    enabled: t === "organization" && groupBy === "user",
  });
  const globalByDateUser = useQuery({
    ...usageGetGlobalByDateUserOptions({ query }),
    enabled: t === "global" && groupBy === "user",
  });

  // --- Entity breakdowns: By Project ---
  const supportsProject = t === "team" || t === "organization" || t === "global";
  const teamByProject = useQuery({
    ...usageGetTeamByProjectOptions({
      path: {
        org_slug: t === "team" ? scope.orgSlug : "",
        team_slug: t === "team" ? scope.teamSlug : "",
      },
      query,
    }),
    enabled: t === "team" && supportsProject,
  });
  const orgByProject = useQuery({
    ...usageGetOrgByProjectOptions({
      path: { slug: t === "organization" ? scope.slug : "" },
      query,
    }),
    enabled: t === "organization" && supportsProject,
  });
  const globalByProject = useQuery({
    ...usageGetGlobalByProjectOptions({ query }),
    enabled: t === "global" && supportsProject,
  });

  // --- Entity breakdowns: By Date+Project ---
  const teamByDateProject = useQuery({
    ...usageGetTeamByDateProjectOptions({
      path: {
        org_slug: t === "team" ? scope.orgSlug : "",
        team_slug: t === "team" ? scope.teamSlug : "",
      },
      query,
    }),
    enabled: t === "team" && groupBy === "project",
  });
  const orgByDateProject = useQuery({
    ...usageGetOrgByDateProjectOptions({
      path: { slug: t === "organization" ? scope.slug : "" },
      query,
    }),
    enabled: t === "organization" && groupBy === "project",
  });
  const globalByDateProject = useQuery({
    ...usageGetGlobalByDateProjectOptions({ query }),
    enabled: t === "global" && groupBy === "project",
  });

  // --- Entity breakdowns: By Team ---
  const supportsTeam = t === "organization" || t === "global";
  const orgByTeam = useQuery({
    ...usageGetOrgByTeamOptions({
      path: { slug: t === "organization" ? scope.slug : "" },
      query,
    }),
    enabled: t === "organization" && supportsTeam,
  });
  const globalByTeam = useQuery({
    ...usageGetGlobalByTeamOptions({ query }),
    enabled: t === "global" && supportsTeam,
  });

  // --- Entity breakdowns: By Date+Team ---
  const orgByDateTeam = useQuery({
    ...usageGetOrgByDateTeamOptions({
      path: { slug: t === "organization" ? scope.slug : "" },
      query,
    }),
    enabled: t === "organization" && groupBy === "team",
  });
  const globalByDateTeam = useQuery({
    ...usageGetGlobalByDateTeamOptions({ query }),
    enabled: t === "global" && groupBy === "team",
  });

  // --- Entity breakdowns: By Org (global only) ---
  const globalByOrg = useQuery({
    ...usageGetGlobalByOrgOptions({ query }),
    enabled: t === "global",
  });
  const globalByDateOrg = useQuery({
    ...usageGetGlobalByDateOrgOptions({ query }),
    enabled: t === "global" && groupBy === "organization",
  });

  // Select the active query for each dimension
  const pick = <T,>(
    ...queries: { data: T | undefined; isLoading: boolean }[]
  ): { data: T | undefined; isLoading: boolean } => {
    for (const q of queries) {
      if (q.data !== undefined || q.isLoading) return q;
    }
    return { data: undefined, isLoading: false };
  };

  return {
    summary: pick<UsageSummaryResponse>(
      apiKeySummary,
      orgSummary,
      projectSummary,
      teamSummary,
      userSummary,
      meSummary,
      globalSummary
    ),
    byDate: pick<DailySpendResponse[]>(
      apiKeyByDate,
      orgByDate,
      projectByDate,
      teamByDate,
      userByDate,
      meByDate,
      globalByDate
    ),
    byModel: pick<ModelSpendResponse[]>(
      apiKeyByModel,
      orgByModel,
      projectByModel,
      teamByModel,
      userByModel,
      meByModel,
      globalByModel
    ),
    byProvider: pick<ProviderSpendResponse[]>(
      apiKeyByProvider,
      orgByProvider,
      projectByProvider,
      teamByProvider,
      userByProvider,
      meByProvider,
      globalByProvider
    ),
    byDateModel: pick<DailyModelSpendResponse[]>(
      apiKeyByDateModel,
      orgByDateModel,
      projectByDateModel,
      teamByDateModel,
      userByDateModel,
      meByDateModel,
      globalByDateModel
    ),
    byDateProvider: pick<DailyProviderSpendResponse[]>(
      apiKeyByDateProvider,
      orgByDateProvider,
      projectByDateProvider,
      teamByDateProvider,
      userByDateProvider,
      meByDateProvider,
      globalByDateProvider
    ),
    byPricingSource: pick<PricingSourceSpendResponse[]>(
      apiKeyByPricingSource,
      orgByPricingSource,
      projectByPricingSource,
      teamByPricingSource,
      userByPricingSource,
      meByPricingSource,
      globalByPricingSource
    ),
    byDatePricingSource: pick<DailyPricingSourceSpendResponse[]>(
      apiKeyByDatePricingSource,
      orgByDatePricingSource,
      projectByDatePricingSource,
      teamByDatePricingSource,
      userByDatePricingSource,
      meByDatePricingSource,
      globalByDatePricingSource
    ),
    byUser: pick<UserSpendResponse[]>(projectByUser, teamByUser, orgByUser, globalByUser),
    byDateUser: pick<DailyUserSpendResponse[]>(
      projectByDateUser,
      teamByDateUser,
      orgByDateUser,
      globalByDateUser
    ),
    byProject: pick<ProjectSpendResponse[]>(teamByProject, orgByProject, globalByProject),
    byDateProject: pick<DailyProjectSpendResponse[]>(
      teamByDateProject,
      orgByDateProject,
      globalByDateProject
    ),
    byTeam: pick<TeamSpendResponse[]>(orgByTeam, globalByTeam),
    byDateTeam: pick<DailyTeamSpendResponse[]>(orgByDateTeam, globalByDateTeam),
    byOrg: pick<OrgSpendResponse[]>(globalByOrg),
    byDateOrg: pick<DailyOrgSpendResponse[]>(globalByDateOrg),
  };
}

const METRIC_CONFIG: Record<
  ChartMetric,
  { label: string; yKey: string; formatter: (v: number) => string }
> = {
  cost: { label: "Cost", yKey: "cost", formatter: formatCurrency },
  tokens: { label: "Tokens", yKey: "tokens", formatter: formatNumber },
  requests: { label: "Requests", yKey: "requests", formatter: formatNumber },
  input_tokens: { label: "Input Tokens", yKey: "input_tokens", formatter: formatNumber },
  output_tokens: { label: "Output Tokens", yKey: "output_tokens", formatter: formatNumber },
  images: { label: "Images", yKey: "images", formatter: formatNumber },
  audio: { label: "Audio (s)", yKey: "audio", formatter: formatNumber },
};

function metricValueFor(
  metric: ChartMetric,
  d: {
    total_cost: number;
    total_tokens: number;
    request_count: number;
    input_tokens: number;
    output_tokens: number;
    image_count: number;
    audio_seconds: number;
  }
): number {
  switch (metric) {
    case "cost":
      return d.total_cost;
    case "tokens":
      return d.total_tokens;
    case "requests":
      return d.request_count;
    case "input_tokens":
      return d.input_tokens;
    case "output_tokens":
      return d.output_tokens;
    case "images":
      return d.image_count;
    case "audio":
      return d.audio_seconds;
  }
}

/** Pivots daily-by-dimension data into { date, [seriesName]: value } for StackedBarChart */
function pivotToMultiLine<T extends { date: string }>(
  data: T[],
  dimensionKey: keyof T,
  metricAccessor: (row: T) => number
): { chartData: Record<string, unknown>[]; series: ChartSeries[] } {
  const dimensions = [...new Set(data.map((d) => String(d[dimensionKey])))];
  const byDate = new Map<string, Record<string, unknown>>();
  for (const row of data) {
    const date = row.date;
    if (!byDate.has(date)) {
      byDate.set(date, { date });
    }
    byDate.get(date)![String(row[dimensionKey])] = metricAccessor(row);
  }
  const chartData = [...byDate.values()].sort((a, b) =>
    (a.date as string).localeCompare(b.date as string)
  );
  const series: ChartSeries[] = dimensions.map((dim, i) => ({
    dataKey: dim,
    name: dim,
    color: CHART_COLORS[i % CHART_COLORS.length],
  }));
  return { chartData, series };
}

/** Like pivotToMultiLine but uses a key extractor function instead of a property key */
function pivotToMultiLineWithKey<T extends { date: string }>(
  data: T[],
  keyExtractor: (row: T) => string,
  metricAccessor: (row: T) => number
): { chartData: Record<string, unknown>[]; series: ChartSeries[] } {
  const dimensions = [...new Set(data.map(keyExtractor))];
  const byDate = new Map<string, Record<string, unknown>>();
  for (const row of data) {
    const date = row.date;
    if (!byDate.has(date)) {
      byDate.set(date, { date });
    }
    byDate.get(date)![keyExtractor(row)] = metricAccessor(row);
  }
  const chartData = [...byDate.values()].sort((a, b) =>
    (a.date as string).localeCompare(b.date as string)
  );
  const series: ChartSeries[] = dimensions.map((dim, i) => ({
    dataKey: dim,
    name: dim,
    color: CHART_COLORS[i % CHART_COLORS.length],
  }));
  return { chartData, series };
}

const userLabel = (d: { user_name?: string | null; user_email?: string | null }) =>
  d.user_name || d.user_email || "Unattributed";
const projectLabel = (d: { project_name?: string | null }) => d.project_name || "Unattributed";
const teamLabel = (d: { team_name?: string | null }) => d.team_name || "Unattributed";
const orgLabel = (d: { org_name?: string | null }) => d.org_name || "Unattributed";

const xFormatter = (date: string) => {
  const d = new Date(date);
  return `${d.getMonth() + 1}/${d.getDate()}`;
};

function EntityDetailRow({
  name,
  subtitle,
  cost,
  metrics,
}: {
  name: string;
  subtitle: string;
  cost: number;
  metrics: {
    request_count: number;
    input_tokens: number;
    output_tokens: number;
    total_tokens: number;
    image_count: number;
    audio_seconds: number;
    character_count: number;
  };
}) {
  return (
    <div className="flex items-center justify-between rounded-lg border p-3">
      <div>
        <div className="font-medium">{name}</div>
        {subtitle && <div className="text-xs text-muted-foreground">{subtitle}</div>}
        <div className="text-xs text-muted-foreground">
          {formatNumber(metrics.request_count)} requests
          {" | "}
          {formatNumber(metrics.input_tokens)} in / {formatNumber(metrics.output_tokens)} out
          {" | "}
          {formatNumber(metrics.total_tokens)} total tokens
          {metrics.image_count > 0 && (
            <>
              {" | "}
              {formatNumber(metrics.image_count)} images
            </>
          )}
          {metrics.audio_seconds > 0 && (
            <>
              {" | "}
              {formatNumber(metrics.audio_seconds)}s audio
              {metrics.character_count > 0 && <> ({formatNumber(metrics.character_count)} chars)</>}
            </>
          )}
        </div>
      </div>
      <Badge variant="secondary" className="font-mono">
        {formatCurrency(cost)}
      </Badge>
    </div>
  );
}

export default function UsageDashboard({ scope }: UsageDashboardProps) {
  const [dateRange, setDateRange] = useState<DateRange>(getDefaultDateRange(30));
  const [chartMetric, setChartMetric] = useState<ChartMetric>("cost");
  const [groupBy, setGroupBy] = useState<GroupBy>("none");

  const groupByOptions = useMemo(() => getGroupByOptions(scope), [scope]);

  // Reset groupBy when scope changes and current value is no longer valid
  useEffect(() => {
    if (!groupByOptions.some((opt) => opt.value === groupBy)) {
      setGroupBy("none");
    }
  }, [groupByOptions, groupBy]);

  const {
    summary,
    byDate,
    byModel,
    byProvider,
    byDateModel,
    byDateProvider,
    byPricingSource,
    byDatePricingSource,
    byUser,
    byDateUser,
    byProject,
    byDateProject,
    byTeam,
    byDateTeam,
    byOrg,
    byDateOrg,
  } = useScopedUsageQueries(scope, dateRange, groupBy);

  const handleMetricChange = useCallback((v: ChartMetric) => setChartMetric(v), []);
  const handleGroupByChange = useCallback((v: GroupBy) => setGroupBy(v), []);

  // --- Single-line chart data (groupBy = none) ---
  const lineChartData = useMemo(() => {
    if (!byDate.data) return [];
    return byDate.data.slice(-30).map((d) => ({
      date: d.date,
      cost: d.total_cost,
      requests: d.request_count,
      tokens: d.total_tokens,
      input_tokens: d.input_tokens,
      output_tokens: d.output_tokens,
      images: d.image_count,
      audio: d.audio_seconds,
    }));
  }, [byDate.data]);

  // --- Multi-line chart data (groupBy != none) ---
  const multiLineData = useMemo(() => {
    if (groupBy === "model" && byDateModel.data) {
      return pivotToMultiLine(byDateModel.data, "model", (row) => metricValueFor(chartMetric, row));
    }
    if (groupBy === "provider" && byDateProvider.data) {
      return pivotToMultiLine(byDateProvider.data, "provider", (row) =>
        metricValueFor(chartMetric, row)
      );
    }
    if (groupBy === "pricing_source" && byDatePricingSource.data) {
      return pivotToMultiLine(byDatePricingSource.data, "pricing_source", (row) =>
        metricValueFor(chartMetric, row)
      );
    }
    if (groupBy === "user" && byDateUser.data) {
      return pivotToMultiLineWithKey(byDateUser.data, userLabel, (row) =>
        metricValueFor(chartMetric, row)
      );
    }
    if (groupBy === "project" && byDateProject.data) {
      return pivotToMultiLineWithKey(byDateProject.data, projectLabel, (row) =>
        metricValueFor(chartMetric, row)
      );
    }
    if (groupBy === "team" && byDateTeam.data) {
      return pivotToMultiLineWithKey(byDateTeam.data, teamLabel, (row) =>
        metricValueFor(chartMetric, row)
      );
    }
    if (groupBy === "organization" && byDateOrg.data) {
      return pivotToMultiLineWithKey(byDateOrg.data, orgLabel, (row) =>
        metricValueFor(chartMetric, row)
      );
    }
    return null;
  }, [
    groupBy,
    byDateModel.data,
    byDateProvider.data,
    byDatePricingSource.data,
    byDateUser.data,
    byDateProject.data,
    byDateTeam.data,
    byDateOrg.data,
    chartMetric,
  ]);

  const modelPieData = useMemo(() => {
    if (!byModel.data) return [];
    return byModel.data.map((m) => ({ name: m.model, value: m.total_cost }));
  }, [byModel.data]);

  const providerPieData = useMemo(() => {
    if (!byProvider.data) return [];
    return byProvider.data.map((p) => ({ name: p.provider, value: p.total_cost }));
  }, [byProvider.data]);

  const pricingSourcePieData = useMemo(() => {
    if (!byPricingSource.data) return [];
    return byPricingSource.data.map((p) => ({ name: p.pricing_source, value: p.total_cost }));
  }, [byPricingSource.data]);

  const userPieData = useMemo(() => {
    if (!byUser.data) return [];
    return byUser.data.map((u) => ({ name: userLabel(u), value: u.total_cost }));
  }, [byUser.data]);

  const projectPieData = useMemo(() => {
    if (!byProject.data) return [];
    return byProject.data.map((p) => ({ name: projectLabel(p), value: p.total_cost }));
  }, [byProject.data]);

  const teamPieData = useMemo(() => {
    if (!byTeam.data) return [];
    return byTeam.data.map((t) => ({ name: teamLabel(t), value: t.total_cost }));
  }, [byTeam.data]);

  const orgPieData = useMemo(() => {
    if (!byOrg.data) return [];
    return byOrg.data.map((o) => ({ name: orgLabel(o), value: o.total_cost }));
  }, [byOrg.data]);

  const sparklineData = useMemo(() => {
    if (!byDate.data || byDate.data.length < 2) return [];
    return byDate.data.slice(-14).map((d) => d.total_cost);
  }, [byDate.data]);

  const costTrend = useMemo(() => {
    if (!byDate.data || byDate.data.length < 2) return null;
    const recent = byDate.data.slice(-7);
    const previous = byDate.data.slice(-14, -7);
    if (previous.length === 0) return null;
    const recentTotal = recent.reduce((sum, d) => sum + d.total_cost, 0);
    const previousTotal = previous.reduce((sum, d) => sum + d.total_cost, 0);
    if (previousTotal === 0) return null;
    return ((recentTotal - previousTotal) / previousTotal) * 100;
  }, [byDate.data]);

  const metricCfg = METRIC_CONFIG[chartMetric];
  const groupByLabel: Record<string, string> = {
    model: "Model",
    provider: "Provider",
    pricing_source: "Pricing Source",
    user: "User",
    project: "Project",
    team: "Team",
    organization: "Organization",
  };
  const chartTitle =
    groupBy === "none"
      ? `${metricCfg.label} Over Time`
      : `${metricCfg.label} Over Time by ${groupByLabel[groupBy]}`;

  const isMultiLineLoading =
    (groupBy === "model" && byDateModel.isLoading) ||
    (groupBy === "provider" && byDateProvider.isLoading) ||
    (groupBy === "pricing_source" && byDatePricingSource.isLoading) ||
    (groupBy === "user" && byDateUser.isLoading) ||
    (groupBy === "project" && byDateProject.isLoading) ||
    (groupBy === "team" && byDateTeam.isLoading) ||
    (groupBy === "organization" && byDateOrg.isLoading);

  // Determine which entity pie charts are available for this scope
  const hasEntityBreakdowns =
    scope.type === "project" ||
    scope.type === "team" ||
    scope.type === "organization" ||
    scope.type === "global";

  return (
    <div className="space-y-6">
      <div className="flex justify-end">
        <DateRangeFilter value={dateRange} onChange={setDateRange} />
      </div>

      {/* Summary Cards */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4 xl:grid-cols-6">
        <StatCard
          title="Total Cost"
          icon={<DollarSign className="h-4 w-4" />}
          isLoading={summary.isLoading}
        >
          <div className="flex items-end justify-between gap-2">
            <StatValue value={formatCurrency(summary.data?.total_cost || 0)} />
            {sparklineData.length > 1 && (
              <div className="flex items-center gap-1">
                <Sparkline
                  data={sparklineData}
                  width={60}
                  height={20}
                  color={costTrend && costTrend < 0 ? "#10b981" : "#0d9488"}
                />
                {costTrend !== null && (
                  <span
                    className={`text-xs font-medium ${costTrend < 0 ? "text-success" : "text-muted-foreground"}`}
                  >
                    {costTrend >= 0 ? "+" : ""}
                    {costTrend.toFixed(0)}%
                  </span>
                )}
              </div>
            )}
          </div>
        </StatCard>

        <StatCard
          title="Total Requests"
          icon={<TrendingUp className="h-4 w-4" />}
          isLoading={summary.isLoading}
        >
          <StatValue value={formatNumber(summary.data?.request_count || 0)} />
        </StatCard>

        <StatCard
          title="Total Tokens"
          icon={<Cpu className="h-4 w-4" />}
          isLoading={summary.isLoading}
        >
          <StatValue value={formatNumber(summary.data?.total_tokens || 0)} />
          {summary.data && (summary.data.input_tokens > 0 || summary.data.output_tokens > 0) && (
            <div className="mt-1 text-xs text-muted-foreground">
              {formatNumber(summary.data.input_tokens)} in /{" "}
              {formatNumber(summary.data.output_tokens)} out
            </div>
          )}
        </StatCard>

        <StatCard
          title="Date Range"
          icon={<Calendar className="h-4 w-4" />}
          isLoading={summary.isLoading}
        >
          {summary.data?.first_request_at ? (
            <div className="text-sm">
              <div>{formatDateTime(summary.data.first_request_at)}</div>
              <div className="text-muted-foreground">to</div>
              <div>
                {formatDateTime(summary.data.last_request_at || summary.data.first_request_at)}
              </div>
            </div>
          ) : (
            <span className="text-muted-foreground">No data</span>
          )}
        </StatCard>

        {summary.data && summary.data.image_count > 0 && (
          <StatCard
            title="Images Generated"
            icon={<ImageIcon className="h-4 w-4" />}
            isLoading={summary.isLoading}
          >
            <StatValue value={formatNumber(summary.data.image_count)} />
          </StatCard>
        )}

        {summary.data && summary.data.audio_seconds > 0 && (
          <StatCard
            title="Audio Seconds"
            icon={<Mic className="h-4 w-4" />}
            isLoading={summary.isLoading}
          >
            <StatValue value={formatNumber(summary.data.audio_seconds)} />
            {summary.data.character_count > 0 && (
              <div className="mt-1 text-xs text-muted-foreground">
                {formatNumber(summary.data.character_count)} characters
              </div>
            )}
          </StatCard>
        )}
      </div>

      {/* Time-series chart with metric + group-by selectors */}
      <Card>
        <CardHeader>
          <div className="flex flex-wrap items-center justify-between gap-4">
            <CardTitle className="flex items-center gap-2">
              <TrendingUp className="h-5 w-5" />
              {chartTitle}
            </CardTitle>
            <div className="flex flex-wrap items-center gap-3">
              <ToggleButtons
                options={METRIC_OPTIONS}
                value={chartMetric}
                onChange={handleMetricChange}
                label="Chart metric"
              />
              <div className="h-4 w-px bg-border" role="separator" />
              <div className="flex items-center gap-1.5">
                <Layers className="h-3.5 w-3.5 text-muted-foreground" />
                <ToggleButtons
                  options={groupByOptions}
                  value={groupBy}
                  onChange={handleGroupByChange}
                  label="Group by"
                />
              </div>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          {byDate.isLoading || isMultiLineLoading ? (
            <div className="h-[240px] animate-pulse rounded bg-muted" />
          ) : groupBy !== "none" && multiLineData && multiLineData.series.length > 0 ? (
            <StackedBarChart
              data={multiLineData.chartData}
              xKey="date"
              series={multiLineData.series}
              height={240}
              formatter={metricCfg.formatter}
              xFormatter={xFormatter}
            />
          ) : !lineChartData.length ? (
            <p className="py-8 text-center text-muted-foreground">
              No usage data for the selected period.
            </p>
          ) : (
            <LineChart
              data={lineChartData}
              xKey="date"
              yKey={metricCfg.yKey}
              height={240}
              formatter={metricCfg.formatter}
              xFormatter={xFormatter}
              showArea={true}
            />
          )}
        </CardContent>
      </Card>

      {/* Pie Charts: Model, Provider & Pricing Source */}
      <div className="grid gap-6 lg:grid-cols-3">
        <PieChartCard
          title="Cost by Model"
          icon={<PieChartIcon className="h-5 w-5" />}
          data={modelPieData}
          isLoading={byModel.isLoading}
          emptyMessage="No model usage data for the selected period."
        />
        <PieChartCard
          title="Cost by Provider"
          icon={<Server className="h-5 w-5" />}
          data={providerPieData}
          isLoading={byProvider.isLoading}
          emptyMessage="No provider usage data for the selected period."
        />
        <PieChartCard
          title="Cost by Pricing Source"
          icon={<Tag className="h-5 w-5" />}
          data={pricingSourcePieData}
          isLoading={byPricingSource.isLoading}
          emptyMessage="No pricing source data for the selected period."
        />
      </div>

      {/* Entity Pie Charts (only for scopes that support entity breakdowns) */}
      {hasEntityBreakdowns && (
        <div className="grid gap-6 lg:grid-cols-3">
          {userPieData.length > 0 && (
            <PieChartCard
              title="Cost by User"
              icon={<User className="h-5 w-5" />}
              data={userPieData}
              isLoading={byUser.isLoading}
              emptyMessage="No user usage data for the selected period."
            />
          )}
          {projectPieData.length > 0 && (
            <PieChartCard
              title="Cost by Project"
              icon={<FolderKanban className="h-5 w-5" />}
              data={projectPieData}
              isLoading={byProject.isLoading}
              emptyMessage="No project usage data for the selected period."
            />
          )}
          {teamPieData.length > 0 && (
            <PieChartCard
              title="Cost by Team"
              icon={<Users className="h-5 w-5" />}
              data={teamPieData}
              isLoading={byTeam.isLoading}
              emptyMessage="No team usage data for the selected period."
            />
          )}
          {orgPieData.length > 0 && (
            <PieChartCard
              title="Cost by Organization"
              icon={<Building2 className="h-5 w-5" />}
              data={orgPieData}
              isLoading={byOrg.isLoading}
              emptyMessage="No organization usage data for the selected period."
            />
          )}
        </div>
      )}

      {/* Model Details Table */}
      {byModel.data && byModel.data.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Cpu className="h-5 w-5" />
              Model Details
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              {byModel.data.map((model) => (
                <EntityDetailRow
                  key={model.model}
                  name={model.model}
                  subtitle=""
                  cost={model.total_cost}
                  metrics={model}
                />
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Provider Details Table */}
      {byProvider.data && byProvider.data.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Server className="h-5 w-5" />
              Provider Details
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              {byProvider.data.map((provider) => (
                <EntityDetailRow
                  key={provider.provider}
                  name={provider.provider}
                  subtitle=""
                  cost={provider.total_cost}
                  metrics={provider}
                />
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Pricing Source Details Table */}
      {byPricingSource.data && byPricingSource.data.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Tag className="h-5 w-5" />
              Pricing Source Details
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              {byPricingSource.data.map((source) => (
                <EntityDetailRow
                  key={source.pricing_source}
                  name={source.pricing_source}
                  subtitle=""
                  cost={source.total_cost}
                  metrics={source}
                />
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      {/* User Details Table */}
      {byUser.data && byUser.data.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <User className="h-5 w-5" />
              User Details
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              {byUser.data.map((u, i) => (
                <EntityDetailRow
                  key={u.user_id || `unattributed-${i}`}
                  name={u.user_name || u.user_email || "Unattributed"}
                  subtitle={u.user_name && u.user_email ? u.user_email : ""}
                  cost={u.total_cost}
                  metrics={u}
                />
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Project Details Table */}
      {byProject.data && byProject.data.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <FolderKanban className="h-5 w-5" />
              Project Details
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              {byProject.data.map((p, i) => (
                <EntityDetailRow
                  key={p.project_id || `unattributed-${i}`}
                  name={p.project_name || "Unattributed"}
                  subtitle=""
                  cost={p.total_cost}
                  metrics={p}
                />
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Team Details Table */}
      {byTeam.data && byTeam.data.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Users className="h-5 w-5" />
              Team Details
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              {byTeam.data.map((t, i) => (
                <EntityDetailRow
                  key={t.team_id || `unattributed-${i}`}
                  name={t.team_name || "Unattributed"}
                  subtitle=""
                  cost={t.total_cost}
                  metrics={t}
                />
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Organization Details Table */}
      {byOrg.data && byOrg.data.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Building2 className="h-5 w-5" />
              Organization Details
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              {byOrg.data.map((o, i) => (
                <EntityDetailRow
                  key={o.org_id || `unattributed-${i}`}
                  name={o.org_name || "Unattributed"}
                  subtitle=""
                  cost={o.total_cost}
                  metrics={o}
                />
              ))}
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}

function PieChartCard({
  title,
  icon,
  data,
  isLoading,
  emptyMessage,
}: {
  title: string;
  icon: React.ReactNode;
  data: { name: string; value: number }[];
  isLoading: boolean;
  emptyMessage: string;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          {icon}
          {title}
        </CardTitle>
      </CardHeader>
      <CardContent>
        {isLoading ? (
          <div className="h-[200px] animate-pulse rounded bg-muted" />
        ) : !data.length ? (
          <p className="py-8 text-center text-muted-foreground">{emptyMessage}</p>
        ) : (
          <>
            <PieChart data={data} height={180} formatter={formatCurrency} />
            <div className="mt-4">
              <ChartLegend
                items={data.map((d, i) => ({
                  name: d.name,
                  value: d.value,
                  color: CHART_COLORS[i % CHART_COLORS.length],
                }))}
                formatter={formatCurrency}
              />
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}
