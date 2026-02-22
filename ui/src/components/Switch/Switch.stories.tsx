import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";

import { Switch } from "./Switch";

const meta: Meta<typeof Switch> = {
  title: "Components/Switch",
  component: Switch,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof Switch>;

function DefaultStory() {
  const [checked, setChecked] = useState(false);
  return (
    <Switch checked={checked} onChange={(e) => setChecked(e.target.checked)} aria-label="Toggle" />
  );
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function CheckedStory() {
  const [checked, setChecked] = useState(true);
  return (
    <Switch checked={checked} onChange={(e) => setChecked(e.target.checked)} aria-label="Toggle" />
  );
}

export const Checked: Story = {
  render: () => <CheckedStory />,
};

function WithLabelStory() {
  const [checked, setChecked] = useState(false);
  return (
    <Switch
      label="Enable notifications"
      checked={checked}
      onChange={(e) => setChecked(e.target.checked)}
    />
  );
}

export const WithLabel: Story = {
  render: () => <WithLabelStory />,
};

function WithLabelAndDescriptionStory() {
  const [checked, setChecked] = useState(true);
  return (
    <div className="w-80">
      <Switch
        label="Show Token Counts"
        description="Display token usage in messages"
        checked={checked}
        onChange={(e) => setChecked(e.target.checked)}
      />
    </div>
  );
}

export const WithLabelAndDescription: Story = {
  render: () => <WithLabelAndDescriptionStory />,
};

export const Disabled: Story = {
  render: () => (
    <div className="space-y-4">
      <Switch label="Disabled unchecked" description="This switch is disabled" disabled />
      <Switch
        label="Disabled checked"
        description="This switch is disabled"
        disabled
        checked
        onChange={() => {}}
      />
    </div>
  ),
};

function SettingsExampleStory() {
  const [settings, setSettings] = useState({
    tokenCounts: true,
    costs: false,
    compact: false,
  });

  return (
    <div className="w-96 space-y-4">
      <Switch
        label="Show Token Counts"
        description="Display token usage in messages"
        checked={settings.tokenCounts}
        onChange={(e) => setSettings({ ...settings, tokenCounts: e.target.checked })}
      />
      <Switch
        label="Show Costs"
        description="Display cost information in messages"
        checked={settings.costs}
        onChange={(e) => setSettings({ ...settings, costs: e.target.checked })}
      />
      <Switch
        label="Compact Messages"
        description="Use compact layout for messages"
        checked={settings.compact}
        onChange={(e) => setSettings({ ...settings, compact: e.target.checked })}
      />
    </div>
  );
}

export const SettingsExample: Story = {
  render: () => <SettingsExampleStory />,
};
