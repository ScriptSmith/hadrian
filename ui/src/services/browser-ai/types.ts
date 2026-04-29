/**
 * Type declarations for the on-device LanguageModel API exposed by recent
 * Chromium browsers (Chrome, Edge, Brave, etc.). Lives on `window` and
 * dedicated workers; not exposed in service workers (see the bridge in
 * `service-worker/browser-ai.ts` for how the SW reaches it).
 *
 * Spec: https://github.com/webmachinelearning/prompt-api
 */

export type LanguageModelAvailability =
  | "available"
  | "downloadable"
  | "downloading"
  | "unavailable";

export interface LanguageModelMonitor {
  /**
   * `loaded` is a fraction in [0, 1], not a byte count. The Prompt API spec
   * normalizes progress and omits `total` for that reason.
   * https://github.com/webmachinelearning/prompt-api?tab=readme-ov-file#download-progress
   */
  addEventListener(type: "downloadprogress", listener: (event: { loaded: number }) => void): void;
  removeEventListener(
    type: "downloadprogress",
    listener: (event: { loaded: number }) => void
  ): void;
}

export interface LanguageModelMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

/**
 * Spec-native tool entry for `LanguageModel.create({ tools: [...] })`. The
 * runtime invokes `execute` whenever the model decides to call this tool;
 * the returned string is fed back as the tool result.
 * https://github.com/webmachinelearning/prompt-api?tab=readme-ov-file#tool-use
 */
export interface LanguageModelTool {
  name: string;
  description?: string;
  inputSchema: object;
  execute: (args: Record<string, unknown>) => Promise<string> | string;
}

export interface LanguageModelExpectedIO {
  type: "text" | "tool-call" | "tool-response" | "image" | "audio";
  languages?: string[];
}

export interface LanguageModelCreateOptions {
  initialPrompts?: LanguageModelMessage[];
  temperature?: number;
  topK?: number;
  tools?: LanguageModelTool[];
  expectedInputs?: LanguageModelExpectedIO[];
  expectedOutputs?: LanguageModelExpectedIO[];
  monitor?: (m: LanguageModelMonitor) => void;
  signal?: AbortSignal;
}

export interface LanguageModelParams {
  defaultTemperature: number;
  maxTemperature: number;
  defaultTopK: number;
  maxTopK: number;
}

export interface LanguageModelPromptOptions {
  signal?: AbortSignal;
  /** JSON Schema constraining the model output at decode time (Chrome 137+). */
  responseConstraint?: object;
  /** Skip auto-injection of the schema into the prompt context. */
  omitResponseConstraintInput?: boolean;
}

export interface LanguageModelSession {
  prompt(
    input: string | LanguageModelMessage[],
    options?: LanguageModelPromptOptions
  ): Promise<string>;
  promptStreaming(
    input: string | LanguageModelMessage[],
    options?: LanguageModelPromptOptions
  ): ReadableStream<string>;
  measureInputUsage(input: string | LanguageModelMessage[]): Promise<number>;
  destroy(): void;
  readonly inputUsage: number;
  readonly inputQuota: number;
}

export interface LanguageModelGlobal {
  availability(): Promise<LanguageModelAvailability>;
  params(): Promise<LanguageModelParams | null>;
  create(options?: LanguageModelCreateOptions): Promise<LanguageModelSession>;
}

declare global {
  var LanguageModel: LanguageModelGlobal | undefined;
}

export {};
