import type { Meta, StoryObj } from "@storybook/react";
import { FileSearchArtifact } from "./FileSearchArtifact";
import type { Artifact } from "@/components/chat-types";

const meta = {
  title: "Chat/Artifacts/FileSearchArtifact",
  component: FileSearchArtifact,
  parameters: {
    layout: "padded",
  },
} satisfies Meta<typeof FileSearchArtifact>;

export default meta;
type Story = StoryObj<typeof meta>;

const makeArtifact = (data: object): Artifact => ({
  id: "search-1",
  type: "file_search",
  title: "Search Results",
  data,
});

export const Default: Story = {
  args: {
    artifact: makeArtifact({
      query: "authentication flow",
      vectorStoreIds: ["vs_123", "vs_456"],
      totalResults: 3,
      results: [
        {
          fileId: "file_abc",
          filename: "auth.ts",
          score: 0.92,
          content:
            "export async function authenticate(credentials: Credentials) {\n  const user = await verifyCredentials(credentials);\n  return generateToken(user);\n}",
        },
        {
          fileId: "file_def",
          filename: "middleware.ts",
          score: 0.78,
          content:
            "export const authMiddleware = async (req, res, next) => {\n  const token = extractToken(req);\n  if (!token) return res.status(401).send('Unauthorized');\n  next();\n};",
        },
        {
          fileId: "file_ghi",
          filename: "README.md",
          score: 0.45,
          content:
            "## Authentication\n\nThis project uses JWT-based authentication. See the auth.ts file for implementation details.",
        },
      ],
    }),
  },
};

export const NoResults: Story = {
  args: {
    artifact: makeArtifact({
      query: "quantum computing algorithms",
      vectorStoreIds: ["vs_123"],
      totalResults: 0,
      results: [],
    }),
  },
};

export const SingleResult: Story = {
  args: {
    artifact: makeArtifact({
      query: "database connection",
      vectorStoreIds: ["vs_789"],
      totalResults: 1,
      results: [
        {
          fileId: "file_xyz",
          filename: "db.ts",
          score: 0.95,
          content: "export const db = new Database(process.env.DATABASE_URL);",
        },
      ],
    }),
  },
};
