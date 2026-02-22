import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";
import { Select } from "./Select";

const meta: Meta = {
  title: "UI/Select",
  component: Select,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div style={{ width: 300 }}>
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

const options = [
  { value: "apple", label: "Apple" },
  { value: "banana", label: "Banana" },
  { value: "cherry", label: "Cherry" },
  { value: "date", label: "Date" },
  { value: "elderberry", label: "Elderberry" },
];

function DefaultStory() {
  const [value, setValue] = useState<string | null>(null);
  return (
    <Select options={options} value={value} onChange={setValue} placeholder="Select a fruit..." />
  );
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function WithValueStory() {
  const [value, setValue] = useState<string | null>("banana");
  return (
    <Select options={options} value={value} onChange={setValue} placeholder="Select a fruit..." />
  );
}

export const WithValue: Story = {
  render: () => <WithValueStory />,
};

function SearchableStory() {
  const [value, setValue] = useState<string | null>(null);
  return (
    <Select
      options={options}
      value={value}
      onChange={setValue}
      placeholder="Search fruits..."
      searchable
    />
  );
}

export const Searchable: Story = {
  render: () => <SearchableStory />,
};

function MultiSelectStory() {
  const [value, setValue] = useState<string[]>([]);
  return (
    <Select
      options={options}
      value={value}
      onChange={(v) => setValue(v as string[])}
      placeholder="Select fruits..."
      multiple
    />
  );
}

export const MultiSelect: Story = {
  render: () => <MultiSelectStory />,
};

function WithDisabledOptionsStory() {
  const [value, setValue] = useState<string | null>(null);
  const optionsWithDisabled = [
    { value: "apple", label: "Apple" },
    { value: "banana", label: "Banana", disabled: true },
    { value: "cherry", label: "Cherry" },
    { value: "date", label: "Date", disabled: true },
  ];
  return (
    <Select
      options={optionsWithDisabled}
      value={value}
      onChange={setValue}
      placeholder="Select a fruit..."
    />
  );
}

export const WithDisabledOptions: Story = {
  render: () => <WithDisabledOptionsStory />,
};

function ErrorStory() {
  const [value, setValue] = useState<string | null>(null);
  return (
    <Select
      options={options}
      value={value}
      onChange={setValue}
      placeholder="Select a fruit..."
      error
    />
  );
}

export const Error: Story = {
  render: () => <ErrorStory />,
};

export const Disabled: Story = {
  render: () => {
    return <Select options={options} value="apple" onChange={() => {}} disabled />;
  },
};
