import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/Card/Card";
import { Input } from "@/components/Input/Input";
import { ThemeToggle } from "@/components/ThemeToggle/ThemeToggle";
import { usePreferences } from "@/preferences/PreferencesProvider";
import { Select } from "@/components/Select/Select";
import { Switch } from "@/components/Switch/Switch";
import { PageHeader } from "@/components/Admin";

export default function SettingsPage() {
  const { preferences, setPreferences } = usePreferences();

  return (
    <div className="p-6">
      <PageHeader title="Settings" description="Configure your preferences and display settings" />

      <div className="max-w-2xl space-y-6">
        <Card>
          <CardHeader>
            <CardTitle>Appearance</CardTitle>
            <CardDescription>Customize how the application looks</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="font-medium">Theme</p>
                <p className="text-sm text-muted-foreground">Choose light, dark, or system theme</p>
              </div>
              <ThemeToggle />
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Chat Settings</CardTitle>
            <CardDescription>Configure chat behavior and defaults</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <Switch
              label="Show Token Counts"
              description="Display token usage in messages"
              checked={preferences.showTokenCounts}
              onChange={(e) => setPreferences({ showTokenCounts: e.target.checked })}
            />

            <Switch
              label="Show Costs"
              description="Display cost information in messages"
              checked={preferences.showCosts}
              onChange={(e) => setPreferences({ showCosts: e.target.checked })}
            />

            <Switch
              label="Compact Messages"
              description="Use compact layout for messages"
              checked={preferences.compactMessages}
              onChange={(e) => setPreferences({ compactMessages: e.target.checked })}
            />

            <div className="space-y-2 pt-2 border-t">
              <label htmlFor="titleGenerationModel" className="text-sm font-medium">
                Title Generation Model
              </label>
              <Input
                id="titleGenerationModel"
                value={preferences.titleGenerationModel}
                onChange={(e) => setPreferences({ titleGenerationModel: e.target.value })}
                placeholder="e.g., openai/gpt-4o-mini"
              />
              <p className="text-xs text-muted-foreground">
                Model used to auto-generate conversation titles. Leave empty to disable LLM title
                generation.
              </p>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Conversation Storage</CardTitle>
            <CardDescription>Configure where conversations are saved</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-2">
              <p className="text-sm font-medium">Default Owner</p>
              <Select
                options={[
                  { value: "user", label: "User (personal)" },
                  { value: "project", label: "Project" },
                ]}
                value={preferences.defaultConversationOwner.type}
                onChange={(value) => {
                  if (value) {
                    setPreferences({
                      defaultConversationOwner: { type: value as "user" | "project" },
                    });
                  }
                }}
              />
              <p className="text-xs text-muted-foreground">
                Choose whether conversations are saved to your user account or a project.
              </p>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
