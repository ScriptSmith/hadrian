/// <reference lib="webworker" />

/**
 * Service-worker side of the Browser AI integration. Intercepts requests for
 * `browser/*` models and routes them through a postMessage bridge to the
 * controlling window: the Prompt API global (`LanguageModel`) is only
 * exposed in window/dedicated-worker scopes, not in service workers.
 */

import type { LanguageModelMessage } from "../services/browser-ai/types";
import {
  BROWSER_AI_PREFIX,
  BROWSER_AI_PROVIDER,
  detectBrowserAiModel,
} from "../services/browser-ai/availability";

declare const self: ServiceWorkerGlobalScope;

export { BROWSER_AI_PREFIX };

interface BridgeAvailabilityReply {
  type: "AVAILABILITY";
  state: "available" | "downloadable" | "downloading" | "unavailable";
}

type BridgeReply =
  | BridgeAvailabilityReply
  | { type: "DOWNLOAD_PROGRESS"; loaded: number }
  | { type: "DELTA"; text: string }
  | { type: "DONE"; inputTokens: number; outputTokens: number }
  | { type: "ABORTED" }
  | { type: "ERROR"; message: string };

interface PromptToolDef {
  name: string;
  description?: string;
  parameters?: object;
}

let availabilityCache: { state: BridgeAvailabilityReply["state"]; checkedAt: number } | null = null;
const AVAILABILITY_TTL_MS = 60_000;

export function isBrowserAiModel(model: unknown): boolean {
  return typeof model === "string" && model.startsWith(BROWSER_AI_PREFIX);
}

async function getClient(clientId: string): Promise<Client | null> {
  // Only the originating tab's window can service the request: its bridge
  // owns the conversation context and abort signal. Falling back to
  // "first window client" cross-routes between tabs.
  if (!clientId) return null;
  const direct = await self.clients.get(clientId);
  return direct ?? null;
}

async function sendToBridge<T extends BridgeReply>(
  client: Client,
  payload:
    | { type: "AVAILABILITY" }
    | {
        type: "PROMPT";
        messages: LanguageModelMessage[];
        temperature?: number;
        topK?: number;
        responseConstraint?: object;
      },
  onMessage?: (reply: BridgeReply, port: MessagePort) => boolean,
  signal?: AbortSignal
): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const channel = new MessageChannel();
    const port = channel.port1;
    let settled = false;

    const cleanup = () => {
      try {
        port.close();
      } catch {
        // ignored
      }
      if (signal && abortHandler) signal.removeEventListener("abort", abortHandler);
    };

    const settle = (fn: () => void) => {
      if (settled) return;
      settled = true;
      cleanup();
      fn();
    };

    let abortHandler: (() => void) | null = null;
    if (signal) {
      abortHandler = () => {
        try {
          port.postMessage({ type: "ABORT" });
        } catch {
          // ignored
        }
        settle(() => reject(new DOMException("Aborted", "AbortError")));
      };
      if (signal.aborted) {
        abortHandler();
      } else {
        signal.addEventListener("abort", abortHandler);
      }
    }

    port.onmessage = (event: MessageEvent) => {
      const reply = event.data as BridgeReply;
      try {
        if (onMessage) {
          const finished = onMessage(reply, port);
          if (finished) {
            settle(() => resolve(reply as T));
          }
          return;
        }
        // No streaming handler: resolve on first reply.
        settle(() => resolve(reply as T));
      } catch (err) {
        // Consumers signal terminal errors by throwing inside `onMessage`
        // (e.g. on an ERROR reply). Catching here is critical: an uncaught
        // throw inside a port.onmessage handler is silently swallowed, so
        // without this the outer Promise would never settle and the SW
        // request would hang until the page is reloaded.
        settle(() => reject(err));
      }
    };
    port.start();

    try {
      client.postMessage({ type: "BROWSER_AI_REQUEST", payload }, [channel.port2]);
    } catch (err) {
      settle(() => reject(err));
    }
  });
}

export async function getCachedAvailability(
  clientId: string
): Promise<BridgeAvailabilityReply["state"]> {
  if (availabilityCache && Date.now() - availabilityCache.checkedAt < AVAILABILITY_TTL_MS) {
    return availabilityCache.state;
  }
  const client = await getClient(clientId);
  if (!client) return "unavailable";
  try {
    const reply = await sendToBridge<BridgeAvailabilityReply>(client, { type: "AVAILABILITY" });
    if (reply.type === "AVAILABILITY") {
      availabilityCache = { state: reply.state, checkedAt: Date.now() };
      return reply.state;
    }
  } catch {
    // Bridge unavailable.
  }
  return "unavailable";
}

/** Append the browser AI model to a `/v1/models` response when supported. */
export async function augmentModelsResponse(
  response: Response,
  clientId: string
): Promise<Response> {
  let availability: BridgeAvailabilityReply["state"];
  try {
    availability = await getCachedAvailability(clientId);
  } catch {
    return response;
  }
  // Only expose the model after the user has explicitly downloaded it via
  // the wizard. Listing it while merely `downloadable` would trigger a
  // multi-GB download on first chat use with no progress indication.
  if (availability !== "available") return response;

  let body: { data?: unknown[]; [k: string]: unknown };
  try {
    body = await response.clone().json();
  } catch {
    return response;
  }
  const data = Array.isArray(body.data) ? body.data : [];

  const detected = detectBrowserAiModel();
  const entry = {
    id: detected.id,
    object: "model",
    created: 0,
    owned_by: BROWSER_AI_PROVIDER,
    source: "static",
    description: `On-device ${detected.vendor} model, runs locally in your browser.`,
    capabilities: { tools: true, vision: false, streaming: true },
    modalities: { input: ["text"], output: ["text"] },
    tasks: ["chat"],
  };

  const augmented = { ...body, data: [...data, entry] };
  const headers = new Headers(response.headers);
  headers.delete("content-length");
  return new Response(JSON.stringify(augmented), {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}

interface ToolDef {
  type?: string;
  name: string;
  description?: string;
  parameters?: object;
}

interface ResponsesPayload {
  model: string;
  input: Array<{
    role?: string;
    type?: string;
    content?: string | Array<{ type: string; text?: string }>;
    [k: string]: unknown;
  }>;
  stream?: boolean;
  temperature?: number;
  top_k?: number;
  max_output_tokens?: number;
  tools?: ToolDef[];
}

interface ChatCompletionsPayload {
  model: string;
  messages: Array<{ role: string; content: string | Array<{ type: string; text?: string }> }>;
  stream?: boolean;
  temperature?: number;
  top_k?: number;
  max_tokens?: number;
  tools?: Array<{ type: string; function?: ToolDef }>;
}

function flattenContent(content: unknown): string {
  if (typeof content === "string") return content;
  if (Array.isArray(content)) {
    return content
      .map((part) => {
        if (typeof part === "string") return part;
        if (part && typeof part === "object") {
          const p = part as { type?: string; text?: string; value?: unknown };
          if (p.type === "input_text" || p.type === "output_text" || p.type === "text") {
            return p.text ?? "";
          }
        }
        return "";
      })
      .join("");
  }
  return "";
}

function inputToMessages(input: ResponsesPayload["input"]): LanguageModelMessage[] {
  const messages: LanguageModelMessage[] = [];

  // function_call_output items reference a prior function_call by call_id.
  // Build a lookup so we can render the result alongside the tool name in
  // the synthetic conversation we feed to the Prompt API.
  const callIdToName = new Map<string, string>();
  for (const item of input) {
    if (
      item.type === "function_call" &&
      typeof item.call_id === "string" &&
      typeof item.name === "string"
    ) {
      callIdToName.set(item.call_id, item.name);
    }
  }

  for (const item of input) {
    if (item.type === "function_call") {
      const name = typeof item.name === "string" ? item.name : "tool";
      const args = typeof item.arguments === "string" ? item.arguments : "{}";
      messages.push({
        role: "assistant",
        content: `<tool_call name="${name}">${args}</tool_call>`,
      });
      continue;
    }
    if (item.type === "function_call_output") {
      const callId = typeof item.call_id === "string" ? item.call_id : "";
      const name = callIdToName.get(callId) ?? "tool";
      const output =
        typeof item.output === "string" ? item.output : JSON.stringify(item.output ?? "");
      messages.push({
        role: "user",
        content: `<tool_result name="${name}">${output}</tool_result>`,
      });
      continue;
    }
    if (item.type && item.type !== "message") continue;
    const role = item.role;
    if (role !== "system" && role !== "user" && role !== "assistant") continue;
    const text = flattenContent(item.content);
    if (!text) continue;
    messages.push({ role, content: text });
  }
  return messages;
}

/** Convert OpenAI-style tool definitions into the bridge's payload shape. */
function extractTools(body: ResponsesPayload | ChatCompletionsPayload): PromptToolDef[] {
  const out: PromptToolDef[] = [];
  for (const t of body.tools ?? []) {
    if (!t) continue;
    // Responses API wraps function tools flat: { type: "function", name, description, parameters }
    // Chat completions wraps them: { type: "function", function: { name, description, parameters } }
    const candidate =
      "function" in t && t.function
        ? t.function
        : (t as { name?: string; description?: string; parameters?: object });
    if (!candidate || typeof candidate.name !== "string") continue;
    const tType = (t as { type?: string }).type;
    if (tType && tType !== "function") continue;
    out.push({
      name: candidate.name,
      description: candidate.description,
      parameters: candidate.parameters,
    });
  }
  return out;
}

/**
 * Polyfill for the spec's native `tools` option, which Chrome rejects at
 * `LanguageModel.create()` today. Instead of asking the model to emit
 * `<tool_call>` markers in free text (which it mutates into markdown
 * fences, drops closing tags, mixes with prose, etc), we describe the
 * tools in the system prompt and force a JSON-shaped reply via
 * `responseConstraint`. Chrome 137+ enforces the schema at decode time,
 * which the swyx and dobidev write-ups identify as the only mechanism
 * that reliably pins format on Gemini Nano.
 */
function injectToolPrompt(
  messages: LanguageModelMessage[],
  tools: PromptToolDef[]
): LanguageModelMessage[] {
  if (tools.length === 0) return messages;
  const toolBlock = tools
    .map((t) => {
      const params = t.parameters ? JSON.stringify(t.parameters) : "{}";
      const desc = t.description ?? "(no description)";
      return `- ${t.name}: ${desc}\n  arguments schema: ${params}`;
    })
    .join("\n\n");

  const instructions = [
    "You can use tools. Your reply will be a JSON object with two optional fields:",
    '  "tool_calls": list of tool invocations, each {"name": "...", "arguments": {...}}',
    '  "text": plain-text reply to the user',
    "",
    "Use tool_calls when you need to run a tool. Use text when you have a final answer. You may use both.",
    "",
    "Available tools:",
    "",
    toolBlock,
    "",
    "Examples (these are entire valid replies):",
    '{"tool_calls":[{"name":"code_interpreter","arguments":{"code":"print(\'hi\')"}}]}',
    '{"tool_calls":[{"name":"code_interpreter","arguments":{"code":"import math\\nprint(math.pi)"}}]}',
    '{"text":"Hello! How can I help?"}',
    '{"text":"Let me check.","tool_calls":[{"name":"wikipedia","arguments":{"action":"search","query":"Einstein"}}]}',
    "",
    'Tool results arrive in the next user message in the form: <tool_result name="TOOL_NAME">...</tool_result>. After receiving tool results, reply with {"text":"..."} containing your final answer.',
  ].join("\n");

  const out = messages.slice();
  const systemIdx = out.findIndex((m) => m.role === "system");
  if (systemIdx >= 0) {
    out[systemIdx] = {
      role: "system",
      content: `${out[systemIdx].content}\n\n${instructions}`,
    };
  } else {
    out.unshift({ role: "system", content: instructions });
  }
  return out;
}

interface ParsedToolCall {
  name: string;
  arguments: string;
}

interface ParsedEnvelope {
  toolCalls: ParsedToolCall[];
  text: string;
}

/**
 * Build the `responseConstraint` schema for a request that has tools.
 * Constrains the model to a `{tool_calls?, text?}` object where every
 * tool name comes from the supplied list. Argument schemas are kept as
 * plain `object` to avoid tripping up Chrome's JSON Schema implementation
 * with per-tool oneOf gymnastics; per-arg validation happens downstream
 * in Hadrian's tool executors.
 */
function buildToolResponseSchema(tools: PromptToolDef[]): object {
  const toolNames = tools.map((t) => t.name);
  return {
    type: "object",
    properties: {
      tool_calls: {
        type: "array",
        items: {
          type: "object",
          properties: {
            name: { type: "string", enum: toolNames },
            arguments: { type: "object" },
          },
          required: ["name", "arguments"],
        },
      },
      text: { type: "string" },
    },
  };
}

/**
 * Parse the constrained JSON envelope returned by the model. Returns
 * empty arrays when the body fails to parse so callers can fall back to
 * a retry path.
 */
function parseEnvelope(raw: string): ParsedEnvelope | null {
  const trimmed = raw.trim();
  if (!trimmed) return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch {
    return null;
  }
  if (!parsed || typeof parsed !== "object") return null;
  const obj = parsed as { tool_calls?: unknown; text?: unknown };
  const toolCalls: ParsedToolCall[] = [];
  if (Array.isArray(obj.tool_calls)) {
    for (const entry of obj.tool_calls) {
      if (!entry || typeof entry !== "object") continue;
      const item = entry as { name?: unknown; arguments?: unknown };
      if (typeof item.name !== "string") continue;
      const args =
        item.arguments && typeof item.arguments === "object"
          ? (item.arguments as Record<string, unknown>)
          : {};
      toolCalls.push({ name: item.name, arguments: JSON.stringify(args) });
    }
  }
  const text = typeof obj.text === "string" ? obj.text : "";
  return { toolCalls, text };
}

function chatMessagesToBridge(
  messages: ChatCompletionsPayload["messages"]
): LanguageModelMessage[] {
  const out: LanguageModelMessage[] = [];
  for (const m of messages) {
    if (m.role !== "system" && m.role !== "user" && m.role !== "assistant") continue;
    const text = flattenContent(m.content);
    if (!text) continue;
    out.push({ role: m.role, content: text });
  }
  return out;
}

function jsonError(message: string, status = 503): Response {
  return new Response(JSON.stringify({ error: { message, type: "browser_ai_error" } }), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function sseHeaders(): HeadersInit {
  return {
    "content-type": "text/event-stream; charset=utf-8",
    "cache-control": "no-cache, no-transform",
    "x-accel-buffering": "no",
  };
}

function sseEvent(name: string, data: unknown): string {
  return `event: ${name}\ndata: ${JSON.stringify(data)}\n\n`;
}

function genId(prefix: string): string {
  return `${prefix}_${Math.random().toString(36).slice(2, 12)}${Date.now().toString(36)}`;
}

/** Handle `/api/v1/responses` for Browser AI. */
export async function handleResponsesRequest(
  request: Request,
  body: ResponsesPayload,
  clientId: string
): Promise<Response> {
  const client = await getClient(clientId);
  if (!client) return jsonError("No active client to handle Browser AI request.");

  let messages = inputToMessages(body.input ?? []);
  if (messages.length === 0) {
    return jsonError("Browser AI requires at least one text message.", 400);
  }

  const tools = extractTools(body);
  const responseId = genId("resp");
  const model = body.model;
  const stream = body.stream !== false;

  if (tools.length > 0) {
    messages = injectToolPrompt(messages, tools);
    return generateToolModeResponse(
      client,
      body,
      messages,
      tools,
      request.signal,
      responseId,
      model,
      stream
    );
  }

  const messageItemId = genId("msg");
  if (!stream) {
    return generateNonStreamingResponse(
      client,
      body,
      messages,
      request.signal,
      responseId,
      messageItemId,
      model
    );
  }
  return generateStreamingResponse(
    client,
    body,
    messages,
    request.signal,
    responseId,
    messageItemId,
    model
  );
}

/**
 * Tool-aware path. Buffers the full generated text from the bridge, parses
 * `<tool_call>` markers, and emits either function_call output items or a
 * single message item depending on what the model produced. Always wraps
 * the result in the Responses-API event sequence so the chat UI sees its
 * normal lifecycle, even though no text is streamed token-by-token.
 */
async function generateToolModeResponse(
  client: Client,
  body: ResponsesPayload,
  messages: LanguageModelMessage[],
  tools: PromptToolDef[],
  signal: AbortSignal,
  responseId: string,
  model: string,
  stream: boolean
): Promise<Response> {
  const schema = buildToolResponseSchema(tools);

  async function runOnce(
    msgs: LanguageModelMessage[]
  ): Promise<{ raw: string; inputTokens: number; outputTokens: number }> {
    let raw = "";
    let inputTokens = 0;
    let outputTokens = 0;
    await sendToBridge<BridgeReply>(
      client,
      {
        type: "PROMPT",
        messages: msgs,
        temperature: body.temperature,
        topK: body.top_k,
        responseConstraint: schema,
      },
      (reply) => {
        if (reply.type === "DELTA") {
          raw += reply.text;
          return false;
        }
        if (reply.type === "DONE") {
          inputTokens = reply.inputTokens;
          outputTokens = reply.outputTokens;
          return true;
        }
        if (reply.type === "ERROR") throw new Error(reply.message);
        if (reply.type === "ABORTED") throw new DOMException("Aborted", "AbortError");
        return false;
      },
      signal
    );
    return { raw, inputTokens, outputTokens };
  }

  let raw: string;
  let inputTokens: number;
  let outputTokens: number;
  try {
    ({ raw, inputTokens, outputTokens } = await runOnce(messages));
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return jsonError(`Browser AI: ${message}`);
  }

  // `responseConstraint` enforces the schema at decode time, so JSON.parse
  // is guaranteed to succeed. The only remaining failure mode is the model
  // emitting `{}` (both fields are optional in the schema), which a retry
  // does not reliably correct. We surface whatever we got — empty case is
  // handled below by falling back to the raw text.
  const envelope = parseEnvelope(raw);
  const toolCalls = envelope?.toolCalls ?? [];
  const text = envelope?.text ?? "";
  const createdAt = Math.floor(Date.now() / 1000);

  const outputItems: Array<Record<string, unknown>> = [];
  for (const call of toolCalls) {
    const fcId = genId("fc");
    outputItems.push({
      id: fcId,
      type: "function_call",
      call_id: fcId,
      name: call.name,
      arguments: call.arguments,
      status: "completed",
    });
  }
  if (text) {
    outputItems.push({
      id: genId("msg"),
      type: "message",
      role: "assistant",
      status: "completed",
      content: [{ type: "output_text", text }],
    });
  }
  if (outputItems.length === 0) {
    // Model returned an empty envelope. Surface the raw output so the
    // user sees what came back rather than a blank turn.
    outputItems.push({
      id: genId("msg"),
      type: "message",
      role: "assistant",
      status: "completed",
      content: [{ type: "output_text", text: raw.trim() || "(no response)" }],
    });
  }

  const completedResponse = {
    id: responseId,
    object: "response",
    created_at: createdAt,
    status: "completed" as const,
    model,
    output: outputItems,
    output_text: text,
    usage: {
      input_tokens: inputTokens,
      output_tokens: outputTokens,
      total_tokens: inputTokens + outputTokens,
    },
  };

  if (!stream) {
    return new Response(JSON.stringify(completedResponse), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  }

  const encoder = new TextEncoder();
  const sseStream = new ReadableStream<Uint8Array>({
    start(controller) {
      const enqueue = (event: string, data: unknown) => {
        controller.enqueue(encoder.encode(sseEvent(event, data)));
      };

      enqueue("response.created", {
        type: "response.created",
        response: { ...completedResponse, status: "in_progress", output: [] },
      });

      let outputIndex = 0;
      for (const item of outputItems) {
        const isFunctionCall = item.type === "function_call";
        enqueue("response.output_item.added", {
          type: "response.output_item.added",
          output_index: outputIndex,
          item: isFunctionCall
            ? { ...item, arguments: "" }
            : { ...item, status: "in_progress", content: [] },
        });

        if (isFunctionCall) {
          enqueue("response.function_call_arguments.delta", {
            type: "response.function_call_arguments.delta",
            item_id: item.id,
            output_index: outputIndex,
            delta: item.arguments,
          });
          enqueue("response.function_call_arguments.done", {
            type: "response.function_call_arguments.done",
            item_id: item.id,
            output_index: outputIndex,
            arguments: item.arguments,
          });
        } else {
          const text = (item.content as Array<{ text: string }>)[0]?.text ?? "";
          enqueue("response.content_part.added", {
            type: "response.content_part.added",
            item_id: item.id,
            output_index: outputIndex,
            content_index: 0,
            part: { type: "output_text", text: "" },
          });
          enqueue("response.output_text.delta", {
            type: "response.output_text.delta",
            item_id: item.id,
            output_index: outputIndex,
            content_index: 0,
            delta: text,
          });
          enqueue("response.output_text.done", {
            type: "response.output_text.done",
            item_id: item.id,
            output_index: outputIndex,
            content_index: 0,
            text,
          });
          enqueue("response.content_part.done", {
            type: "response.content_part.done",
            item_id: item.id,
            output_index: outputIndex,
            content_index: 0,
            part: { type: "output_text", text },
          });
        }

        enqueue("response.output_item.done", {
          type: "response.output_item.done",
          output_index: outputIndex,
          item,
        });
        outputIndex += 1;
      }

      enqueue("response.completed", {
        type: "response.completed",
        response: completedResponse,
      });
      controller.enqueue(encoder.encode("data: [DONE]\n\n"));
      controller.close();
    },
  });

  return new Response(sseStream, { status: 200, headers: sseHeaders() });
}

async function generateNonStreamingResponse(
  client: Client,
  body: ResponsesPayload,
  messages: LanguageModelMessage[],
  signal: AbortSignal,
  responseId: string,
  messageItemId: string,
  model: string
): Promise<Response> {
  let outputText = "";
  let inputTokens = 0;
  let outputTokens = 0;
  try {
    await sendToBridge<BridgeReply>(
      client,
      {
        type: "PROMPT",
        messages,
        temperature: body.temperature,
        topK: body.top_k,
      },
      (reply) => {
        if (reply.type === "DELTA") {
          outputText += reply.text;
          return false;
        }
        if (reply.type === "DONE") {
          inputTokens = reply.inputTokens;
          outputTokens = reply.outputTokens;
          return true;
        }
        if (reply.type === "ERROR") {
          throw new Error(reply.message);
        }
        if (reply.type === "ABORTED") {
          throw new DOMException("Aborted", "AbortError");
        }
        return false;
      },
      signal
    );
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return jsonError(`Browser AI: ${message}`);
  }

  return new Response(
    JSON.stringify({
      id: responseId,
      object: "response",
      created_at: Math.floor(Date.now() / 1000),
      status: "completed",
      model,
      output: [
        {
          id: messageItemId,
          type: "message",
          role: "assistant",
          status: "completed",
          content: [{ type: "output_text", text: outputText }],
        },
      ],
      output_text: outputText,
      usage: {
        input_tokens: inputTokens,
        output_tokens: outputTokens,
        total_tokens: inputTokens + outputTokens,
      },
    }),
    { status: 200, headers: { "content-type": "application/json" } }
  );
}

async function generateStreamingResponse(
  client: Client,
  body: ResponsesPayload,
  messages: LanguageModelMessage[],
  signal: AbortSignal,
  responseId: string,
  messageItemId: string,
  model: string
): Promise<Response> {
  const encoder = new TextEncoder();
  const createdAt = Math.floor(Date.now() / 1000);

  const stream = new ReadableStream<Uint8Array>({
    async start(controller) {
      const enqueue = (event: string, data: unknown) => {
        controller.enqueue(encoder.encode(sseEvent(event, data)));
      };

      const baseResponse = {
        id: responseId,
        object: "response",
        created_at: createdAt,
        model,
        status: "in_progress",
        output: [] as unknown[],
      };

      enqueue("response.created", { type: "response.created", response: baseResponse });

      enqueue("response.output_item.added", {
        type: "response.output_item.added",
        output_index: 0,
        item: {
          id: messageItemId,
          type: "message",
          role: "assistant",
          status: "in_progress",
          content: [],
        },
      });

      enqueue("response.content_part.added", {
        type: "response.content_part.added",
        item_id: messageItemId,
        output_index: 0,
        content_index: 0,
        part: { type: "output_text", text: "" },
      });

      let outputText = "";
      let inputTokens = 0;
      let outputTokens = 0;
      let downloading = false;

      try {
        await sendToBridge<BridgeReply>(
          client,
          {
            type: "PROMPT",
            messages,
            temperature: body.temperature,
            topK: body.top_k,
          },
          (reply) => {
            if (reply.type === "DOWNLOAD_PROGRESS") {
              if (!downloading) {
                downloading = true;
                enqueue("response.browser_ai.download.started", {
                  type: "response.browser_ai.download.started",
                });
              }
              enqueue("response.browser_ai.download.progress", {
                type: "response.browser_ai.download.progress",
                loaded: reply.loaded,
              });
              return false;
            }
            if (reply.type === "DELTA") {
              outputText += reply.text;
              enqueue("response.output_text.delta", {
                type: "response.output_text.delta",
                item_id: messageItemId,
                output_index: 0,
                content_index: 0,
                delta: reply.text,
              });
              return false;
            }
            if (reply.type === "DONE") {
              inputTokens = reply.inputTokens;
              outputTokens = reply.outputTokens;
              return true;
            }
            if (reply.type === "ERROR") {
              throw new Error(reply.message);
            }
            if (reply.type === "ABORTED") {
              throw new DOMException("Aborted", "AbortError");
            }
            return false;
          },
          signal
        );
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        enqueue("response.error", {
          type: "response.error",
          error: { message: `Browser AI: ${message}`, type: "browser_ai_error" },
        });
        controller.close();
        return;
      }

      enqueue("response.output_text.done", {
        type: "response.output_text.done",
        item_id: messageItemId,
        output_index: 0,
        content_index: 0,
        text: outputText,
      });

      enqueue("response.content_part.done", {
        type: "response.content_part.done",
        item_id: messageItemId,
        output_index: 0,
        content_index: 0,
        part: { type: "output_text", text: outputText },
      });

      enqueue("response.output_item.done", {
        type: "response.output_item.done",
        output_index: 0,
        item: {
          id: messageItemId,
          type: "message",
          role: "assistant",
          status: "completed",
          content: [{ type: "output_text", text: outputText }],
        },
      });

      enqueue("response.completed", {
        type: "response.completed",
        response: {
          ...baseResponse,
          status: "completed",
          output: [
            {
              id: messageItemId,
              type: "message",
              role: "assistant",
              status: "completed",
              content: [{ type: "output_text", text: outputText }],
            },
          ],
          output_text: outputText,
          usage: {
            input_tokens: inputTokens,
            output_tokens: outputTokens,
            total_tokens: inputTokens + outputTokens,
          },
        },
      });

      controller.enqueue(encoder.encode("data: [DONE]\n\n"));
      controller.close();
    },
  });

  return new Response(stream, { status: 200, headers: sseHeaders() });
}

/** Handle `/v1/chat/completions` for Browser AI. */
export async function handleChatCompletionsRequest(
  request: Request,
  body: ChatCompletionsPayload,
  clientId: string
): Promise<Response> {
  const client = await getClient(clientId);
  if (!client) return jsonError("No active client to handle Browser AI request.");

  let messages = chatMessagesToBridge(body.messages ?? []);
  if (messages.length === 0) {
    return jsonError("Browser AI requires at least one text message.", 400);
  }

  const tools = extractTools(body);
  if (tools.length > 0) {
    messages = injectToolPrompt(messages, tools);
  }
  const id = genId("chatcmpl");
  const created = Math.floor(Date.now() / 1000);
  const model = body.model;
  const stream = body.stream === true;

  if (!stream) {
    let outputText = "";
    let inputTokens = 0;
    let outputTokens = 0;
    try {
      await sendToBridge<BridgeReply>(
        client,
        {
          type: "PROMPT",
          messages,
          temperature: body.temperature,
          topK: body.top_k,
        },
        (reply) => {
          if (reply.type === "DELTA") {
            outputText += reply.text;
            return false;
          }
          if (reply.type === "DONE") {
            inputTokens = reply.inputTokens;
            outputTokens = reply.outputTokens;
            return true;
          }
          if (reply.type === "ERROR") throw new Error(reply.message);
          if (reply.type === "ABORTED") throw new DOMException("Aborted", "AbortError");
          return false;
        },
        request.signal
      );
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      return jsonError(`Browser AI: ${message}`);
    }

    return new Response(
      JSON.stringify({
        id,
        object: "chat.completion",
        created,
        model,
        choices: [
          {
            index: 0,
            message: { role: "assistant", content: outputText },
            finish_reason: "stop",
          },
        ],
        usage: {
          prompt_tokens: inputTokens,
          completion_tokens: outputTokens,
          total_tokens: inputTokens + outputTokens,
        },
      }),
      { status: 200, headers: { "content-type": "application/json" } }
    );
  }

  const encoder = new TextEncoder();
  const sseStream = new ReadableStream<Uint8Array>({
    async start(controller) {
      const writeChunk = (chunk: Record<string, unknown>) => {
        controller.enqueue(encoder.encode(`data: ${JSON.stringify(chunk)}\n\n`));
      };

      writeChunk({
        id,
        object: "chat.completion.chunk",
        created,
        model,
        choices: [{ index: 0, delta: { role: "assistant" }, finish_reason: null }],
      });

      let outputText = "";
      let inputTokens = 0;
      let outputTokens = 0;

      try {
        await sendToBridge<BridgeReply>(
          client,
          {
            type: "PROMPT",
            messages,
            temperature: body.temperature,
            topK: body.top_k,
          },
          (reply) => {
            if (reply.type === "DELTA") {
              outputText += reply.text;
              writeChunk({
                id,
                object: "chat.completion.chunk",
                created,
                model,
                choices: [{ index: 0, delta: { content: reply.text }, finish_reason: null }],
              });
              return false;
            }
            if (reply.type === "DONE") {
              inputTokens = reply.inputTokens;
              outputTokens = reply.outputTokens;
              return true;
            }
            if (reply.type === "ERROR") throw new Error(reply.message);
            if (reply.type === "ABORTED") throw new DOMException("Aborted", "AbortError");
            return false;
          },
          request.signal
        );
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        writeChunk({
          error: { message: `Browser AI: ${message}`, type: "browser_ai_error" },
        });
        controller.close();
        return;
      }

      writeChunk({
        id,
        object: "chat.completion.chunk",
        created,
        model,
        choices: [{ index: 0, delta: {}, finish_reason: "stop" }],
        usage: {
          prompt_tokens: inputTokens,
          completion_tokens: outputTokens,
          total_tokens: inputTokens + outputTokens,
        },
      });

      // Acknowledge unused output for type-checker: `outputText` tracks the
      // streamed text but we don't replay it at the end.
      void outputText;

      controller.enqueue(encoder.encode("data: [DONE]\n\n"));
      controller.close();
    },
  });

  return new Response(sseStream, { status: 200, headers: sseHeaders() });
}

export type { ResponsesPayload, ChatCompletionsPayload };
