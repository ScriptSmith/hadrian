import { describe, it, expect } from "vitest";
import {
  getShortModelName,
  extractUserMessageText,
  filterMessagesForModel,
  messagesToInputItems,
  aggregateUsage,
  groupByRound,
  getSortedRounds,
  formatRoundTranscript,
  formatSingleRound,
  parseJsonFromResponse,
} from "../utils";
import type { ChatMessage } from "../../types";

describe("getShortModelName", () => {
  it("extracts last segment after slash", () => {
    expect(getShortModelName("anthropic/claude-3-opus")).toBe("claude-3-opus");
    expect(getShortModelName("openai/gpt-4")).toBe("gpt-4");
  });

  it("returns original if no slash", () => {
    expect(getShortModelName("gpt-4")).toBe("gpt-4");
  });

  it("handles multiple slashes", () => {
    expect(getShortModelName("provider/namespace/model-name")).toBe("model-name");
  });

  it("handles empty string", () => {
    expect(getShortModelName("")).toBe("");
  });

  it("handles trailing slash", () => {
    // split("/").pop() returns empty string, fallback returns original
    expect(getShortModelName("provider/")).toBe("provider/");
  });
});

describe("extractUserMessageText", () => {
  it("returns string content as-is", () => {
    expect(extractUserMessageText("Hello world")).toBe("Hello world");
  });

  it("extracts text from multimodal content", () => {
    const content = [
      { type: "input_text", text: "Describe this image" },
      { type: "image", source: { type: "base64", data: "..." } },
    ];
    expect(extractUserMessageText(content)).toBe("Describe this image");
  });

  it("returns empty string if no input_text in multimodal content", () => {
    const content = [{ type: "image", source: { type: "base64", data: "..." } }];
    expect(extractUserMessageText(content)).toBe("");
  });

  it("handles empty array", () => {
    expect(extractUserMessageText([])).toBe("");
  });
});

describe("filterMessagesForModel", () => {
  const messages: ChatMessage[] = [
    { id: "1", role: "user", content: "Hello", timestamp: new Date() },
    { id: "2", role: "assistant", content: "Hi!", model: "gpt-4", timestamp: new Date() },
    { id: "3", role: "user", content: "How are you?", timestamp: new Date() },
    { id: "4", role: "assistant", content: "Good!", model: "claude-3", timestamp: new Date() },
  ];

  it("returns all messages when historyMode is 'all'", () => {
    const result = filterMessagesForModel(messages, "gpt-4", "all");
    expect(result).toEqual(messages);
  });

  it("filters to user messages and same-model assistant messages", () => {
    const result = filterMessagesForModel(messages, "gpt-4", "same-model");
    expect(result).toHaveLength(3);
    expect(result.map((m) => m.id)).toEqual(["1", "2", "3"]);
  });

  it("filters for a different model", () => {
    const result = filterMessagesForModel(messages, "claude-3", "same-model");
    expect(result).toHaveLength(3);
    expect(result.map((m) => m.id)).toEqual(["1", "3", "4"]);
  });
});

describe("messagesToInputItems", () => {
  it("maps messages to input format", () => {
    const messages: ChatMessage[] = [
      { id: "1", role: "user", content: "Hello", timestamp: new Date() },
      { id: "2", role: "assistant", content: "Hi there!", model: "gpt-4", timestamp: new Date() },
    ];

    const result = messagesToInputItems(messages);
    expect(result).toEqual([
      { role: "user", content: "Hello" },
      { role: "assistant", content: "Hi there!" },
    ]);
  });

  it("preserves string content", () => {
    const content = "Test content";
    const messages: ChatMessage[] = [{ id: "1", role: "user", content, timestamp: new Date() }];

    const result = messagesToInputItems(messages);
    expect(result[0].content).toBe(content);
  });
});

describe("aggregateUsage", () => {
  it("sums usage from array of items", () => {
    const items = [
      { usage: { inputTokens: 100, outputTokens: 50, totalTokens: 150, cost: 0.01 } },
      { usage: { inputTokens: 200, outputTokens: 100, totalTokens: 300, cost: 0.02 } },
    ];

    const result = aggregateUsage(items);
    expect(result).toEqual({
      inputTokens: 300,
      outputTokens: 150,
      totalTokens: 450,
      cost: 0.03,
    });
  });

  it("handles items with missing usage", () => {
    const items = [
      { usage: { inputTokens: 100, outputTokens: 50, totalTokens: 150, cost: 0.01 } },
      {},
      { usage: undefined },
    ];

    const result = aggregateUsage(items);
    expect(result).toEqual({
      inputTokens: 100,
      outputTokens: 50,
      totalTokens: 150,
      cost: 0.01,
    });
  });

  it("adds additional usage items", () => {
    const items = [{ usage: { inputTokens: 100, outputTokens: 50, totalTokens: 150, cost: 0.01 } }];
    const additional = { inputTokens: 50, outputTokens: 25, totalTokens: 75, cost: 0.005 };

    const result = aggregateUsage(items, additional);
    expect(result).toEqual({
      inputTokens: 150,
      outputTokens: 75,
      totalTokens: 225,
      cost: 0.015,
    });
  });

  it("handles undefined additional usage", () => {
    const items = [{ usage: { inputTokens: 100, outputTokens: 50, totalTokens: 150, cost: 0.01 } }];

    const result = aggregateUsage(items, undefined, undefined);
    expect(result).toEqual({
      inputTokens: 100,
      outputTokens: 50,
      totalTokens: 150,
      cost: 0.01,
    });
  });

  it("returns zero values for empty input", () => {
    const result = aggregateUsage([]);
    expect(result).toEqual({
      inputTokens: 0,
      outputTokens: 0,
      totalTokens: 0,
      cost: 0,
    });
  });
});

describe("groupByRound", () => {
  it("groups items by round number", () => {
    const items = [
      { round: 1, content: "a" },
      { round: 2, content: "b" },
      { round: 1, content: "c" },
      { round: 2, content: "d" },
    ];

    const result = groupByRound(items);
    expect(result).toEqual({
      1: [
        { round: 1, content: "a" },
        { round: 1, content: "c" },
      ],
      2: [
        { round: 2, content: "b" },
        { round: 2, content: "d" },
      ],
    });
  });

  it("handles empty array", () => {
    const result = groupByRound([]);
    expect(result).toEqual({});
  });

  it("handles single item", () => {
    const items = [{ round: 5, data: "test" }];
    const result = groupByRound(items);
    expect(result).toEqual({ 5: [{ round: 5, data: "test" }] });
  });
});

describe("getSortedRounds", () => {
  it("returns sorted round numbers", () => {
    const byRound = { 3: ["a"], 1: ["b"], 5: ["c"], 2: ["d"] };
    const result = getSortedRounds(byRound);
    expect(result).toEqual([1, 2, 3, 5]);
  });

  it("handles empty object", () => {
    const result = getSortedRounds({});
    expect(result).toEqual([]);
  });
});

describe("formatRoundTranscript", () => {
  const items = [
    { round: 1, model: "anthropic/claude-3", content: "Opening from Claude" },
    { round: 1, model: "openai/gpt-4", content: "Opening from GPT" },
    { round: 2, model: "anthropic/claude-3", content: "Rebuttal from Claude" },
    { round: 2, model: "openai/gpt-4", content: "Rebuttal from GPT" },
  ];

  it("formats transcript with round labels", () => {
    const result = formatRoundTranscript(items, {
      getRoundLabel: (round) => (round === 1 ? "Opening Statements" : `Round ${round}`),
    });

    expect(result).toContain("### Opening Statements");
    expect(result).toContain("### Round 2");
    expect(result).toContain("**claude-3**");
    expect(result).toContain("**gpt-4**");
    expect(result).toContain("Opening from Claude");
    expect(result).toContain("Rebuttal from GPT");
  });

  it("uses custom item labels when provided", () => {
    const result = formatRoundTranscript(items, {
      getRoundLabel: (round) => `Round ${round}`,
      getItemLabel: (model) => (model.includes("claude") ? "Pro" : "Con"),
    });

    expect(result).toContain("(Pro)");
    expect(result).toContain("(Con)");
  });

  it("falls back to short model name for item label", () => {
    const result = formatRoundTranscript(items, {
      getRoundLabel: (round) => `Round ${round}`,
      getItemLabel: () => undefined,
    });

    expect(result).toContain("(claude-3)");
    expect(result).toContain("(gpt-4)");
  });

  it("separates rounds with horizontal rules", () => {
    const result = formatRoundTranscript(items, {
      getRoundLabel: (round) => `Round ${round}`,
    });

    expect(result).toContain("\n\n---\n\n");
  });
});

describe("formatSingleRound", () => {
  const items = [
    { round: 1, model: "anthropic/claude-3", content: "Content A" },
    { round: 1, model: "openai/gpt-4", content: "Content B" },
    { round: 2, model: "anthropic/claude-3", content: "Content C" },
  ];

  it("formats only items from the specified round", () => {
    const result = formatSingleRound(items, 1);

    expect(result).toContain("**claude-3**");
    expect(result).toContain("**gpt-4**");
    expect(result).toContain("Content A");
    expect(result).toContain("Content B");
    expect(result).not.toContain("Content C");
  });

  it("uses custom item labels when provided", () => {
    const result = formatSingleRound(items, 1, (model) =>
      model.includes("claude") ? "Expert A" : "Expert B"
    );

    expect(result).toContain("(Expert A)");
    expect(result).toContain("(Expert B)");
  });

  it("returns empty string for round with no items", () => {
    const result = formatSingleRound(items, 99);
    expect(result).toBe("");
  });
});

describe("parseJsonFromResponse", () => {
  it("parses raw JSON object", () => {
    const response = '{"key": "value", "number": 42}';
    const result = parseJsonFromResponse<{ key: string; number: number }>(response);
    expect(result).toEqual({ key: "value", number: 42 });
  });

  it("parses raw JSON array", () => {
    const response = '[1, 2, 3, "four"]';
    const result = parseJsonFromResponse<(number | string)[]>(response);
    expect(result).toEqual([1, 2, 3, "four"]);
  });

  it("extracts JSON from markdown code block", () => {
    const response = `Here's the data:
\`\`\`json
{"roles": {"model1": "Expert", "model2": "Critic"}}
\`\`\`
That's the role assignment.`;

    const result = parseJsonFromResponse<{ roles: Record<string, string> }>(response);
    expect(result).toEqual({ roles: { model1: "Expert", model2: "Critic" } });
  });

  it("extracts JSON from code block without json annotation", () => {
    const response = `\`\`\`
{"simple": true}
\`\`\``;

    const result = parseJsonFromResponse<{ simple: boolean }>(response);
    expect(result).toEqual({ simple: true });
  });

  it("extracts JSON surrounded by text", () => {
    const response = `Based on my analysis, here is the result: {"score": 85, "passed": true} I hope this helps!`;
    const result = parseJsonFromResponse<{ score: number; passed: boolean }>(response);
    expect(result).toEqual({ score: 85, passed: true });
  });

  it("returns null for invalid JSON", () => {
    const response = "This response has no JSON at all.";
    const result = parseJsonFromResponse<unknown>(response);
    expect(result).toBeNull();
  });

  it("returns null for malformed JSON", () => {
    const response = '{"broken": true, missing_quote: bad}';
    const result = parseJsonFromResponse<unknown>(response);
    expect(result).toBeNull();
  });

  it("handles nested JSON objects", () => {
    const response = `\`\`\`json
{
  "subtasks": [
    {"id": 1, "description": "Task A"},
    {"id": 2, "description": "Task B"}
  ],
  "metadata": {"created": "2024-01-01"}
}
\`\`\``;

    const result = parseJsonFromResponse<{
      subtasks: { id: number; description: string }[];
      metadata: { created: string };
    }>(response);

    expect(result?.subtasks).toHaveLength(2);
    expect(result?.subtasks[0].description).toBe("Task A");
    expect(result?.metadata.created).toBe("2024-01-01");
  });
});
