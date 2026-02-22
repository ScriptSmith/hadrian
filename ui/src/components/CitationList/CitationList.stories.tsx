import type { Meta, StoryObj } from "@storybook/react";

import { CitationList, type Citation } from "./CitationList";

const meta: Meta<typeof CitationList> = {
  title: "Components/CitationList",
  component: CitationList,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div className="w-[400px]">
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof CitationList>;

const mockFileCitations: Citation[] = [
  {
    id: "1",
    type: "file",
    fileId: "file_abc123",
    filename: "q3_financial_report.pdf",
    snippet:
      "Revenue increased by 15% compared to Q2, driven primarily by expansion into new markets and increased customer retention rates.",
    score: 0.92,
  },
  {
    id: "2",
    type: "file",
    fileId: "file_def456",
    filename: "market_analysis_2024.docx",
    snippet:
      "Market trends indicate sustained growth in the enterprise segment, with particular strength in cloud services.",
    score: 0.85,
  },
  {
    id: "3",
    type: "file",
    fileId: "file_ghi789",
    filename: "competitor_research.md",
    snippet:
      "Key competitors have announced similar product offerings, though differentiation remains strong.",
    score: 0.71,
  },
];

const mockUrlCitations: Citation[] = [
  {
    id: "4",
    type: "url",
    url: "https://example.com/industry-trends-2024",
    title: "Industry Trends Report 2024",
    snippet:
      "The latest industry report shows significant growth in AI adoption across enterprise customers.",
    score: 0.88,
  },
  {
    id: "5",
    type: "url",
    url: "https://news.example.com/market-update",
    title: "Q3 Market Update",
    snippet: "Stock prices reached new highs following positive earnings announcements.",
    score: 0.76,
  },
];

const mockChunkCitations: Citation[] = [
  {
    id: "6",
    type: "chunk",
    fileId: "file_jkl012",
    filename: "technical_documentation.md",
    chunkIndex: 3,
    content: `## API Authentication

The API uses Bearer token authentication. Include the token in the Authorization header:

\`\`\`
Authorization: Bearer <your-token>
\`\`\`

Tokens expire after 24 hours and must be refreshed.`,
    tokenCount: 45,
    score: 0.95,
  },
  {
    id: "7",
    type: "chunk",
    fileId: "file_mno345",
    filename: "user_guide.md",
    chunkIndex: 12,
    content:
      "To configure the integration, navigate to Settings > Integrations and click 'Add New'. Follow the on-screen wizard to complete the setup process.",
    tokenCount: 28,
    score: 0.82,
  },
];

// Basic stories
export const FileCitations: Story = {
  args: {
    citations: mockFileCitations,
  },
};

export const UrlCitations: Story = {
  args: {
    citations: mockUrlCitations,
  },
};

export const ChunkCitations: Story = {
  args: {
    citations: mockChunkCitations,
  },
};

export const MixedCitations: Story = {
  args: {
    citations: [
      ...mockFileCitations.slice(0, 2),
      ...mockUrlCitations.slice(0, 1),
      ...mockChunkCitations.slice(0, 1),
    ],
  },
};

// Edge cases
export const SingleCitation: Story = {
  args: {
    citations: [mockFileCitations[0]],
  },
};

export const ManyCitations: Story = {
  args: {
    citations: [
      ...mockFileCitations,
      ...mockUrlCitations,
      ...mockChunkCitations,
      {
        id: "8",
        type: "file",
        fileId: "file_extra1",
        filename: "additional_source.pdf",
        score: 0.65,
      },
      {
        id: "9",
        type: "file",
        fileId: "file_extra2",
        filename: "supporting_document.txt",
        score: 0.58,
      },
    ],
    maxVisible: 3,
  },
};

export const NoScores: Story = {
  args: {
    citations: [
      {
        id: "1",
        type: "file",
        fileId: "file_abc",
        filename: "document.pdf",
        snippet: "Some content from the document...",
      },
      {
        id: "2",
        type: "url",
        url: "https://example.com",
        title: "Example Page",
      },
    ],
  },
};

export const CompactMode: Story = {
  args: {
    citations: [...mockFileCitations, ...mockUrlCitations],
    compact: true,
  },
};

export const EmptyCitations: Story = {
  args: {
    citations: [],
  },
};

// Interactive stories
export const WithClickHandlers: Story = {
  args: {
    citations: mockFileCitations,
    onFileClick: (fileId, chunkId) => {
      console.log("File clicked:", fileId, chunkId);
      alert(`Navigate to file: ${fileId}`);
    },
  },
};

export const UrlWithClickHandler: Story = {
  args: {
    citations: mockUrlCitations,
    onUrlClick: (url) => {
      console.log("URL clicked:", url);
      alert(`Would open: ${url}`);
    },
  },
};
