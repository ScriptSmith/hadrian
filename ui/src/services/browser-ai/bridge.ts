import { getAvailability, getLanguageModel } from "./availability";
import type { LanguageModelMessage, LanguageModelSession } from "./types";

/**
 * Window-side bridge that responds to LanguageModel requests from the
 * service worker. The Prompt API (`globalThis.LanguageModel`) is only
 * exposed in window/dedicated-worker scopes, so we relay calls from the
 * SW through this bridge over a `MessageChannel` per request.
 *
 * Tools are not passed through to the model: the spec defines a native
 * `tools` option, but Chrome rejects sessions that supply one
 * ("the device is unable to create a session to run the model"). Until
 * that ships, the SW polyfills tools by injecting their descriptions into
 * the system prompt and parsing `<tool_call>` markers from the streamed
 * text. This bridge stays intentionally tool-agnostic so it works on every
 * Chromium channel that ships the Prompt API.
 */

interface PromptRequestPayload {
  type: "PROMPT";
  messages: LanguageModelMessage[];
  temperature?: number;
  topK?: number;
  /**
   * JSON Schema for `responseConstraint`. When set the bridge runs the
   * non-streaming `prompt()` API: the model output is forced to match the
   * schema, and partial chunks would be malformed JSON anyway.
   */
  responseConstraint?: object;
}

interface AvailabilityRequestPayload {
  type: "AVAILABILITY";
}

type BridgeRequest = PromptRequestPayload | AvailabilityRequestPayload;

export function installBrowserAiBridge(): () => void {
  if (typeof navigator === "undefined" || !("serviceWorker" in navigator)) {
    return () => {};
  }

  const handler = (event: MessageEvent) => {
    const data = event.data as { type?: string; payload?: BridgeRequest } | null;
    if (!data || data.type !== "BROWSER_AI_REQUEST" || !data.payload) return;
    const port = event.ports?.[0];
    if (!port) return;
    handleRequest(port, data.payload).catch((err: unknown) => {
      const message = err instanceof Error ? err.message : String(err);
      try {
        port.postMessage({ type: "ERROR", message });
        port.close();
      } catch {
        // Port already closed.
      }
    });
  };

  navigator.serviceWorker.addEventListener("message", handler);
  return () => navigator.serviceWorker.removeEventListener("message", handler);
}

async function handleRequest(port: MessagePort, payload: BridgeRequest): Promise<void> {
  if (payload.type === "AVAILABILITY") {
    port.postMessage({ type: "AVAILABILITY", state: await getAvailability() });
    port.close();
    return;
  }

  if (payload.type === "PROMPT") {
    await handlePrompt(port, payload);
    return;
  }

  port.postMessage({
    type: "ERROR",
    message: `Unknown bridge request type: ${(payload as { type?: string }).type}`,
  });
  port.close();
}

async function handlePrompt(port: MessagePort, payload: PromptRequestPayload): Promise<void> {
  const lm = getLanguageModel();
  if (!lm) {
    port.postMessage({
      type: "ERROR",
      message: "Browser AI is not available in this browser.",
    });
    port.close();
    return;
  }

  const abort = new AbortController();
  let session: LanguageModelSession | null = null;
  // Tear down the on-device session on cancel. Chrome's LanguageModel
  // implementation does not always honour AbortSignal mid-prompt, so an
  // abort that only fires the controller can leave the call hanging
  // indefinitely. Destroying the session forces it to release.
  abort.signal.addEventListener("abort", () => {
    try {
      session?.destroy();
    } catch {
      // ignored
    }
  });
  port.addEventListener("message", (event: MessageEvent) => {
    if ((event.data as { type?: string } | null)?.type === "ABORT") {
      abort.abort();
    }
  });
  port.start();

  const systemMessages = payload.messages.filter((m) => m.role === "system");
  const conversation = payload.messages.filter((m) => m.role !== "system");

  try {
    session = await lm.create({
      initialPrompts: systemMessages.length > 0 ? systemMessages : undefined,
      temperature: payload.temperature,
      topK: payload.topK,
      monitor(m) {
        m.addEventListener("downloadprogress", (event) => {
          port.postMessage({ type: "DOWNLOAD_PROGRESS", loaded: event.loaded });
        });
      },
      signal: abort.signal,
    });

    // Count input tokens across system + conversation messages. The earlier
    // version only measured `conversation`, which understated usage whenever
    // a system prompt was supplied (every Hadrian chat turn).
    let inputTokens = 0;
    try {
      inputTokens = await session.measureInputUsage(payload.messages);
    } catch (err) {
      // measureInputUsage may not be implemented on every channel.
      console.debug("[browser-ai] measureInputUsage(input) failed", err);
    }

    let outputText = "";
    if (payload.responseConstraint) {
      // Constrained output: token chunks would be malformed JSON, so use
      // the non-streaming API and surface the full response as one delta.
      outputText = await session.prompt(conversation, {
        signal: abort.signal,
        responseConstraint: payload.responseConstraint,
      });
      if (outputText) port.postMessage({ type: "DELTA", text: outputText });
    } else {
      const stream = session.promptStreaming(conversation, { signal: abort.signal });
      const reader = stream.getReader();
      let cumulative = "";

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        // Most Chromium channels stream deltas; older channels streamed
        // cumulative text. Detect and normalise to deltas.
        let delta: string;
        if (
          value.length >= cumulative.length &&
          value.startsWith(cumulative) &&
          cumulative.length > 0
        ) {
          delta = value.slice(cumulative.length);
          cumulative = value;
        } else {
          delta = value;
          cumulative += value;
        }
        if (!delta) continue;
        outputText += delta;
        port.postMessage({ type: "DELTA", text: delta });
      }
    }

    // measureInputUsage of an assistant message also counts role-framing
    // tokens (a few per message). Subtract the framing baseline so the
    // reported output count tracks the generated text rather than the
    // wrapper. Falls back to ~4 chars/token when the API isn't available.
    let outputTokens = 0;
    try {
      const [withText, baseline] = await Promise.all([
        session.measureInputUsage([{ role: "assistant", content: outputText }]),
        session.measureInputUsage([{ role: "assistant", content: "" }]),
      ]);
      outputTokens = Math.max(0, withText - baseline);
    } catch (err) {
      console.debug("[browser-ai] measureInputUsage(output) failed", err);
      outputTokens = Math.max(1, Math.ceil(outputText.length / 4));
    }

    port.postMessage({ type: "DONE", inputTokens, outputTokens });
  } catch (err: unknown) {
    if (abort.signal.aborted) {
      port.postMessage({ type: "ABORTED" });
    } else {
      const message = err instanceof Error ? err.message : String(err);
      port.postMessage({ type: "ERROR", message });
    }
  } finally {
    session?.destroy();
    try {
      port.close();
    } catch {
      // Already closed.
    }
  }
}
