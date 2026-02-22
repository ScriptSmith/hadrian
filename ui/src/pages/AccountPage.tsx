import { useMutation, useQuery } from "@tanstack/react-query";
import { Download, Trash2, User, AlertTriangle, HardDrive } from "lucide-react";

import { meDeleteMutation, meExportOptions } from "@/api/generated/@tanstack/react-query.gen";
import { useAuth } from "@/auth";
import { Button } from "@/components/Button/Button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/Card/Card";
import { useToast } from "@/components/Toast/Toast";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";
import { exportAllIndexedDBData, deleteIndexedDBDatabase } from "@/hooks/useIndexedDB";

// localStorage keys used by the app
const LOCAL_STORAGE_KEYS = ["hadrian-auth", "hadrian-mcp-servers", "hadrian-preferences"] as const;

/** Export all localStorage data for Hadrian keys */
function exportLocalStorageData(): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const key of LOCAL_STORAGE_KEYS) {
    try {
      const value = localStorage.getItem(key);
      if (value) {
        result[key] = JSON.parse(value);
      }
    } catch {
      // If parsing fails, store as raw string
      const value = localStorage.getItem(key);
      if (value) {
        result[key] = value;
      }
    }
  }
  return result;
}

/** Clear all Hadrian-related localStorage data */
function clearLocalStorageData(): void {
  for (const key of LOCAL_STORAGE_KEYS) {
    localStorage.removeItem(key);
  }
}

export default function AccountPage() {
  const { user, logout } = useAuth();
  const { toast } = useToast();
  const confirm = useConfirm();

  // Export data query (only fetch when triggered)
  const { refetch: fetchExport, isFetching: isExporting } = useQuery({
    ...meExportOptions(),
    enabled: false, // Don't auto-fetch
  });

  // Delete account mutation
  const deleteMutation = useMutation({
    ...meDeleteMutation(),
    onSuccess: (data) => {
      toast({
        title: "Account deleted",
        description: `Your account and ${data.conversations_deleted} conversations, ${data.api_keys_deleted} API keys have been permanently deleted.`,
        type: "success",
      });
      // Log out after deletion
      logout();
    },
    onError: (error) => {
      toast({
        title: "Failed to delete account",
        description: String(error),
        type: "error",
      });
    },
  });

  const handleExportData = async () => {
    try {
      // Fetch server-side data
      const serverResult = await fetchExport();

      // Gather local data
      const localStorageData = exportLocalStorageData();
      const indexedDBData = await exportAllIndexedDBData();

      // Combine all data
      const exportData = {
        server_data: serverResult.data ?? null,
        local_data: {
          localStorage: localStorageData,
          indexedDB: indexedDBData,
        },
        exported_at: new Date().toISOString(),
      };

      // Download as JSON file
      const blob = new Blob([JSON.stringify(exportData, null, 2)], {
        type: "application/json",
      });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `hadrian-data-export-${new Date().toISOString().split("T")[0]}.json`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);

      toast({
        title: "Data exported",
        description: "Your server and local data has been downloaded as a JSON file.",
        type: "success",
      });
    } catch (error) {
      toast({
        title: "Failed to export data",
        description: String(error),
        type: "error",
      });
    }
  };

  const handleClearLocalData = async () => {
    const confirmed = await confirm({
      title: "Clear local data?",
      message:
        "This will clear all locally stored data including conversations, MCP server configurations, and preferences. Server-side data will not be affected. You may be logged out.",
      confirmLabel: "Clear Local Data",
      variant: "destructive",
    });

    if (confirmed) {
      try {
        // Clear IndexedDB (conversations)
        await deleteIndexedDBDatabase();
        // Clear localStorage
        clearLocalStorageData();

        toast({
          title: "Local data cleared",
          description: "All locally stored data has been removed from this browser.",
          type: "success",
        });

        // Refresh the page to reset app state
        window.location.reload();
      } catch (error) {
        toast({
          title: "Failed to clear local data",
          description: String(error),
          type: "error",
        });
      }
    }
  };

  const handleDeleteAccount = async () => {
    const confirmed = await confirm({
      title: "Delete your account?",
      message:
        "This will permanently delete your account and all associated data including conversations, API keys, and usage history. Local browser data will also be cleared. This action cannot be undone.",
      confirmLabel: "Delete Account",
      variant: "destructive",
    });

    if (confirmed) {
      // Second confirmation for extra safety
      const doubleConfirmed = await confirm({
        title: "Are you absolutely sure?",
        message:
          "All your data will be permanently deleted from the server and this browser. You will be logged out immediately. There is no way to recover your account.",
        confirmLabel: "Yes, delete everything",
        variant: "destructive",
      });

      if (doubleConfirmed) {
        // Clear local data first (before logout clears auth state)
        try {
          await deleteIndexedDBDatabase();
          clearLocalStorageData();
        } catch {
          // Continue with server deletion even if local clear fails
        }
        deleteMutation.mutate({});
      }
    }
  };

  return (
    <div className="p-6 max-w-2xl mx-auto">
      <div className="mb-8">
        <h1 className="text-2xl font-semibold">Account Settings</h1>
        <p className="text-muted-foreground mt-1">Manage your account and data</p>
      </div>

      {/* Profile Information */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <User className="h-5 w-5" />
            Profile
          </CardTitle>
          <CardDescription>Your account information</CardDescription>
        </CardHeader>
        <CardContent>
          <dl className="space-y-3">
            {user?.name && (
              <div className="flex justify-between">
                <dt className="text-muted-foreground">Name</dt>
                <dd className="font-medium">{user.name}</dd>
              </div>
            )}
            {user?.email && (
              <div className="flex justify-between">
                <dt className="text-muted-foreground">Email</dt>
                <dd className="font-medium">{user.email}</dd>
              </div>
            )}
            {user?.id && (
              <div className="flex justify-between">
                <dt className="text-muted-foreground">User ID</dt>
                <dd className="font-mono text-sm text-muted-foreground">{user.id}</dd>
              </div>
            )}
          </dl>
        </CardContent>
      </Card>

      {/* Data Export */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Download className="h-5 w-5" />
            Export Your Data
          </CardTitle>
          <CardDescription>
            Download all your personal data including profile, conversations, API keys, usage
            history, and local browser data (GDPR Article 15)
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Button onClick={handleExportData} disabled={isExporting} variant="outline">
            <Download className="mr-2 h-4 w-4" />
            {isExporting ? "Exporting..." : "Export Data"}
          </Button>
        </CardContent>
      </Card>

      {/* Local Data Management */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <HardDrive className="h-5 w-5" />
            Local Browser Data
          </CardTitle>
          <CardDescription>
            Manage data stored locally in your browser including cached conversations, MCP server
            configurations, and preferences
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-3">
            <p className="text-sm text-muted-foreground">
              Local data is stored in your browser and includes:
            </p>
            <ul className="text-sm text-muted-foreground list-disc list-inside space-y-1">
              <li>Cached conversations (IndexedDB)</li>
              <li>Audio cache (OPFS)</li>
              <li>MCP server configurations</li>
              <li>Theme and UI preferences</li>
              <li>Authentication tokens</li>
            </ul>
            <Button onClick={handleClearLocalData} variant="outline">
              <Trash2 className="mr-2 h-4 w-4" />
              Clear Local Data
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* Delete Account */}
      <Card className="border-destructive/50">
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-destructive">
            <AlertTriangle className="h-5 w-5" />
            Danger Zone
          </CardTitle>
          <CardDescription>
            Permanently delete your account and all associated data (GDPR Article 17)
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-4">
            <h4 className="font-medium text-destructive mb-2">Delete Account</h4>
            <p className="text-sm text-muted-foreground mb-4">
              Once you delete your account, there is no going back. This will permanently delete:
            </p>
            <ul className="text-sm text-muted-foreground mb-4 list-disc list-inside space-y-1">
              <li>Your user profile</li>
              <li>All conversations and chat history</li>
              <li>All API keys you created</li>
              <li>All dynamic providers you configured</li>
              <li>All usage records and history</li>
              <li>All local browser data (cached conversations, MCP configs, preferences)</li>
            </ul>
            <Button
              variant="danger"
              onClick={handleDeleteAccount}
              disabled={deleteMutation.isPending}
            >
              <Trash2 className="mr-2 h-4 w-4" />
              {deleteMutation.isPending ? "Deleting..." : "Delete Account"}
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
