import type { Meta, StoryObj } from "@storybook/react";
import { HighlightedCode } from "./HighlightedCode";

const meta = {
  title: "Chat/HighlightedCode",
  component: HighlightedCode,
  parameters: {
    layout: "padded",
  },
} satisfies Meta<typeof HighlightedCode>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Python: Story = {
  args: {
    code: `import pandas as pd
import numpy as np

def analyze(data: list[float]) -> dict:
    """Compute summary statistics."""
    arr = np.array(data)
    return {
        "mean": float(arr.mean()),
        "std": float(arr.std()),
        "median": float(np.median(arr)),
    }

result = analyze([1, 2, 3, 4, 5])
print(result)`,
    language: "python",
    showLanguage: true,
  },
};

export const JavaScript: Story = {
  args: {
    code: `const fetchUsers = async () => {
  const response = await fetch('/api/users');
  const data = await response.json();
  return data.users.filter(u => u.active);
};

fetchUsers().then(console.log);`,
    language: "javascript",
    showLanguage: true,
  },
};

export const SQL: Story = {
  args: {
    code: `SELECT u.name, COUNT(o.id) AS order_count
FROM users u
LEFT JOIN orders o ON o.user_id = u.id
WHERE u.created_at > '2024-01-01'
GROUP BY u.name
ORDER BY order_count DESC
LIMIT 10;`,
    language: "sql",
    showLanguage: true,
  },
};

export const Compact: Story = {
  args: {
    code: `data = [1, 2, 3, 4, 5]
result = sum(data) / len(data)
print(f"Average: {result}")`,
    language: "python",
    compact: true,
    showCopy: false,
  },
};

export const WithCopyButton: Story = {
  args: {
    code: `console.log("Hello, world!");`,
    language: "javascript",
    showCopy: true,
  },
};

export const WithMaxHeight: Story = {
  args: {
    code: Array(50)
      .fill(null)
      .map((_, i) => `console.log("Line ${i + 1}");`)
      .join("\n"),
    language: "javascript",
    showLanguage: true,
    maxHeight: "200px",
  },
};

export const UnknownLanguage: Story = {
  args: {
    code: "Some plain text content\nwith multiple lines\nand no highlighting",
    language: "custom-lang",
  },
};
