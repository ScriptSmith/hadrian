import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { http, HttpResponse, delay } from "msw";
import SsoGroupMappingsPage from "./SsoGroupMappingsPage";
import type {
  SsoGroupMapping,
  SsoGroupMappingListResponse,
  Organization,
  Team,
  TeamListResponse,
  TestMappingResponse,
  ExportResponse,
  ImportResponse,
} from "@/api/generated/types.gen";
import { ToastProvider } from "@/components/Toast/Toast";
import { ConfirmDialogProvider } from "@/components/ConfirmDialog/ConfirmDialog";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      staleTime: Infinity,
    },
  },
});

const meta: Meta<typeof SsoGroupMappingsPage> = {
  title: "Admin/SsoGroupMappingsPage",
  component: SsoGroupMappingsPage,
  parameters: {
    layout: "fullscreen",
    a11y: {
      config: {
        rules: [{ id: "heading-order", enabled: false }],
      },
    },
  },
  decorators: [
    (Story, context) => {
      // Clear cache before each story
      queryClient.clear();
      const route = context.parameters.route || "/admin/organizations/acme-corp/sso-group-mappings";
      return (
        <QueryClientProvider client={queryClient}>
          <ToastProvider>
            <ConfirmDialogProvider>
              <MemoryRouter initialEntries={[route]}>
                <Routes>
                  <Route
                    path="/admin/organizations/:orgSlug/sso-group-mappings"
                    element={
                      <div className="min-h-screen bg-background">
                        <Story />
                      </div>
                    }
                  />
                </Routes>
              </MemoryRouter>
            </ConfirmDialogProvider>
          </ToastProvider>
        </QueryClientProvider>
      );
    },
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

const mockOrg: Organization = {
  id: "org-123",
  name: "Acme Corp",
  slug: "acme-corp",
  created_at: "2024-01-01T00:00:00Z",
  updated_at: "2024-01-01T00:00:00Z",
};

const mockTeams: Team[] = [
  {
    id: "team-1",
    name: "Engineering",
    slug: "engineering",
    org_id: "org-123",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "team-2",
    name: "Marketing",
    slug: "marketing",
    org_id: "org-123",
    created_at: "2024-01-02T00:00:00Z",
    updated_at: "2024-01-02T00:00:00Z",
  },
  {
    id: "team-3",
    name: "Finance",
    slug: "finance",
    org_id: "org-123",
    created_at: "2024-01-03T00:00:00Z",
    updated_at: "2024-01-03T00:00:00Z",
  },
];

const mockMappings: SsoGroupMapping[] = [
  {
    id: "mapping-1",
    sso_connection_name: "default",
    idp_group: "Engineering",
    org_id: "org-123",
    team_id: "team-1",
    role: "member",
    priority: 0,
    created_at: "2024-01-10T00:00:00Z",
    updated_at: "2024-01-10T00:00:00Z",
  },
  {
    id: "mapping-2",
    sso_connection_name: "default",
    idp_group: "Engineering-Leads",
    org_id: "org-123",
    team_id: "team-1",
    role: "admin",
    priority: 10,
    created_at: "2024-01-11T00:00:00Z",
    updated_at: "2024-01-11T00:00:00Z",
  },
  {
    id: "mapping-3",
    sso_connection_name: "default",
    idp_group: "marketing@company.com",
    org_id: "org-123",
    team_id: "team-2",
    role: null,
    priority: 0,
    created_at: "2024-01-12T00:00:00Z",
    updated_at: "2024-01-12T00:00:00Z",
  },
  {
    id: "mapping-4",
    sso_connection_name: "azure-ad",
    idp_group: "Finance-Team",
    org_id: "org-123",
    team_id: "team-3",
    role: "viewer",
    priority: 5,
    created_at: "2024-01-13T00:00:00Z",
    updated_at: "2024-01-13T00:00:00Z",
  },
];

const mockMappingsResponse: SsoGroupMappingListResponse = {
  data: mockMappings,
  pagination: {
    total: mockMappings.length,
    limit: 50,
    offset: 0,
  },
};

const mockTeamsResponse: TeamListResponse = {
  data: mockTeams,
  pagination: {
    total: mockTeams.length,
    limit: 50,
    offset: 0,
  },
};

const emptyMappingsResponse: SsoGroupMappingListResponse = {
  data: [],
  pagination: {
    total: 0,
    limit: 50,
    offset: 0,
  },
};

// Mock test response - resolves Engineering to team-1, leaves unknown groups unmapped
const mockTestResponse: TestMappingResponse = {
  resolved: [
    {
      idp_group: "Engineering",
      team_id: "team-1",
      team_name: "Engineering",
      role: "member",
    },
    {
      idp_group: "Engineering-Leads",
      team_id: "team-1",
      team_name: "Engineering",
      role: "admin",
    },
  ],
  unmapped_groups: ["Unknown-Group", "Another-Unknown"],
};

// Mock export response (JSON format)
const mockExportResponse: ExportResponse = {
  organization: "acme-corp",
  exported_at: "2024-01-15T10:00:00Z",
  mappings: mockMappings.map((m) => ({
    idp_group: m.idp_group,
    team_id: m.team_id,
    team_name:
      mockTeams.find((t) => t.id === m.team_id)?.name || (m.team_id ? "Unknown" : undefined),
    role: m.role,
    priority: m.priority,
    sso_connection_name: m.sso_connection_name,
  })),
};

// Mock CSV export
const mockCsvExport = `idp_group,team_id,team_name,role,priority,sso_connection_name
Engineering,team-1,Engineering,member,0,default
Engineering-Leads,team-1,Engineering,admin,10,default
marketing@company.com,team-2,Marketing,,0,default
Finance-Team,team-3,Finance,viewer,5,azure-ad`;

// Mock import response
const mockImportResponse: ImportResponse = {
  created: 3,
  updated: 1,
  skipped: 0,
  errors: [],
};

const _mockImportWithErrorsResponse: ImportResponse = {
  created: 2,
  updated: 0,
  skipped: 1,
  errors: [{ index: 3, idp_group: "Invalid-Group", error: "Team 'nonexistent-team' not found" }],
};

const baseHandlers = [
  http.get("*/admin/v1/organizations/acme-corp", () => {
    return HttpResponse.json(mockOrg);
  }),
  http.get("*/admin/v1/organizations/acme-corp/teams", () => {
    return HttpResponse.json(mockTeamsResponse);
  }),
  http.post("*/admin/v1/organizations/acme-corp/sso-group-mappings/test", () => {
    return HttpResponse.json(mockTestResponse);
  }),
  http.get("*/admin/v1/organizations/acme-corp/sso-group-mappings/export", ({ request }) => {
    const url = new URL(request.url);
    const format = url.searchParams.get("format");
    if (format === "csv") {
      return new HttpResponse(mockCsvExport, {
        headers: {
          "Content-Type": "text/csv; charset=utf-8",
          "Content-Disposition": 'attachment; filename="sso-group-mappings-acme-corp.csv"',
        },
      });
    }
    return HttpResponse.json(mockExportResponse);
  }),
  http.post("*/admin/v1/organizations/acme-corp/sso-group-mappings/import", () => {
    return HttpResponse.json(mockImportResponse);
  }),
];

export const WithMappings: Story = {
  parameters: {
    msw: {
      handlers: [
        ...baseHandlers,
        http.get("*/admin/v1/organizations/acme-corp/sso-group-mappings", () => {
          return HttpResponse.json(mockMappingsResponse);
        }),
      ],
    },
  },
};

export const FilteredByConnection: Story = {
  parameters: {
    route: "/admin/organizations/acme-corp/sso-group-mappings?connection=default",
    msw: {
      handlers: [
        ...baseHandlers,
        http.get("*/admin/v1/organizations/acme-corp/sso-group-mappings", () => {
          return HttpResponse.json(mockMappingsResponse);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  parameters: {
    msw: {
      handlers: [
        ...baseHandlers,
        http.get("*/admin/v1/organizations/acme-corp/sso-group-mappings", () => {
          return HttpResponse.json(emptyMappingsResponse);
        }),
      ],
    },
  },
};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        ...baseHandlers,
        http.get("*/admin/v1/organizations/acme-corp/sso-group-mappings", async () => {
          await delay("infinite");
          return HttpResponse.json(mockMappingsResponse);
        }),
      ],
    },
  },
};

export const Error: Story = {
  parameters: {
    msw: {
      handlers: [
        ...baseHandlers,
        http.get("*/admin/v1/organizations/acme-corp/sso-group-mappings", () => {
          return HttpResponse.json({ error: "Internal server error" }, { status: 500 });
        }),
      ],
    },
  },
};
